//! Tauri app layer. Commands here only pass data to and from the engine, and read files
//! from disk (the engine never touches the file system). No geometry, toolpath or G-code
//! logic belongs in this file.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use tauri::{Manager, State};
use wham_engine::{
    BoxMode, BreakReport, CutWidthStep, DimensionClick, DimensionInfo, DisplayData, DisplayShape,
    Document, Draft, DxfInfo, GenerateReport, GridArea, ImportReport, JoinReport, MachineSetting,
    MachineSettings, Machines, Operation, OperationState, PickedTool, Playback, Point, Project,
    ProjectTemplates, Shape, Snap, SnapOptions, SvgInfo, TemplateSetting, Tool, ToolDrawing,
    ToolSetting, Tooling, ToolpathDisplay, TransformOp,
};

/// The open project, shared by all commands.
struct AppState(Mutex<Project>);

/// Where the open project was last saved or opened from (None until it is first saved).
struct OpenFile(Mutex<Option<PathBuf>>);

/// A settings file of this computer (shared by every project), held in memory with the path it
/// is kept at (None if the settings folder couldn't be found: changes then last until WHAM_CAM
/// closes).
struct SettingsFile<T> {
    value: T,
    path: Option<PathBuf>,
    /// What it is, for messages ("tool list", "machine list").
    what: &'static str,
    to_text: fn(&T) -> String,
    /// A problem found when loading, shown to the user once.
    problem: Option<String>,
}

/// This computer's master tool list and tool libraries (`tooling.json`).
struct ToolingState(Mutex<SettingsFile<Tooling>>);

/// This computer's machines (`machines.json`).
struct MachinesState(Mutex<SettingsFile<Machines>>);

/// This computer's project templates (`project_templates.json`).
struct TemplatesState(Mutex<SettingsFile<ProjectTemplates>>);

/// Loads a settings file from the app's settings folder. A missing file means an empty one.
/// A file that can't be read is kept aside (renamed `.damaged`), never overwritten.
fn load_settings<T: Default>(
    dir: Option<&Path>,
    file_name: &str,
    what: &'static str,
    from_text: fn(&str) -> Result<T, String>,
    to_text: fn(&T) -> String,
) -> SettingsFile<T> {
    let Some(dir) = dir else {
        return SettingsFile {
            value: T::default(),
            path: None,
            what,
            to_text,
            problem: Some(format!(
                "WHAM_CAM couldn't find its settings folder, so changes to the {what} won't be kept."
            )),
        };
    };
    let path = dir.join(file_name);
    let (value, problem) = match std::fs::read_to_string(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (T::default(), None),
        Err(e) => (
            T::default(),
            Some(format!("Could not read the {what} {}: {e}", path.display())),
        ),
        Ok(text) => match from_text(&text) {
            Ok(v) => (v, None),
            Err(e) => {
                let mut aside = path.clone().into_os_string();
                aside.push(".damaged");
                let _ = std::fs::rename(&path, &aside);
                (
                    T::default(),
                    Some(format!(
                        "{e} It was kept as {} and an empty {what} started.",
                        PathBuf::from(aside).display()
                    )),
                )
            }
        },
    };
    SettingsFile {
        value,
        path: Some(path),
        what,
        to_text,
        problem,
    }
}

/// Runs a change on a settings file and writes it. On any error nothing changes.
fn change_settings<T: Clone, R>(
    file: &Mutex<SettingsFile<T>>,
    change: impl FnOnce(&mut T) -> Result<R, String>,
) -> Result<R, String> {
    let mut file = file.lock().unwrap();
    let mut value = file.value.clone();
    let result = change(&mut value)?;
    if let Some(path) = &file.path {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
        }
        write_safely(path, &(file.to_text)(&value))
            .map_err(|e| format!("Could not save the {} {}: {e}", file.what, path.display()))?;
    }
    file.value = value;
    Ok(result)
}

/// Runs a change on the tooling and writes the file.
fn change_tooling<T>(
    state: &State<ToolingState>,
    change: impl FnOnce(&mut Tooling) -> Result<T, String>,
) -> Result<T, String> {
    change_settings(&state.0, change)
}

/// Everything the interface needs to redraw after a change.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct View {
    document: Document,
    display: DisplayData,
    can_undo: bool,
    can_redo: bool,
    /// Unsaved changes since the last save or open.
    modified: bool,
    /// The project file's name (e.g. "bracket.whamcam"), or None if never saved.
    file_name: Option<String>,
    /// Toolpath state of each operation (for the tree).
    operations: Vec<OperationState>,
    /// Toolpaths to draw.
    toolpaths: Vec<ToolpathDisplay>,
    /// This computer's master tool list and tool libraries.
    tooling: Tooling,
    /// This computer's machines.
    machines: Machines,
    /// This computer's project templates.
    templates: ProjectTemplates,
    /// Layers hidden from view (not saved).
    hidden_layers: Vec<String>,
    /// The dimensions as they are now (kind and value).
    dimensions: Vec<DimensionInfo>,
    /// Whether dimensions are shown (not saved).
    dimensions_visible: bool,
}

fn view(
    p: &Project,
    file: Option<&Path>,
    tooling: &Tooling,
    machines: &Machines,
    templates: &ProjectTemplates,
) -> View {
    View {
        document: p.document().clone(),
        display: p.display().clone(),
        can_undo: p.can_undo(),
        can_redo: p.can_redo(),
        modified: p.is_modified(),
        file_name: file
            .and_then(Path::file_name)
            .map(|n| n.to_string_lossy().into_owned()),
        operations: p.operation_states(),
        toolpaths: p.toolpaths_display(),
        tooling: tooling.clone(),
        machines: machines.clone(),
        templates: templates.clone(),
        hidden_layers: p.hidden_layers(),
        dimensions: p.dimensions(),
        dimensions_visible: p.dimensions_visible(),
    }
}

/// The project name for a file: its name without the extension.
fn project_name(path: &Path) -> String {
    path.file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".into())
}

/// Reads a file as text. Non-UTF-8 bytes (older DXFs use Windows code pages) are replaced;
/// they only ever appear in text such as layer names.
fn read_text(path: &str) -> Result<String, String> {
    std::fs::read(path)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .map_err(|e| format!("Could not read {path}: {e}"))
}

#[tauri::command]
fn get_view(
    state: State<AppState>,
    file: State<OpenFile>,
    tooling: State<ToolingState>,
    machines: State<MachinesState>,
    templates: State<TemplatesState>,
) -> View {
    view(
        &state.0.lock().unwrap(),
        file.0.lock().unwrap().as_deref(),
        &tooling.0.lock().unwrap().value,
        &machines.0.lock().unwrap().value,
        &templates.0.lock().unwrap().value,
    )
}

// --- Project templates and machines -------------------------------------------------------------
// The open project keeps a copy of its template's and machine's settings. After every change in
// the Setup tab, the settings it hasn't changed for itself follow (spec 5.7, 5.9).

/// Lets the open project follow the templates and machines as they are now.
fn follow_setup(
    state: &State<AppState>,
    templates: &State<TemplatesState>,
    machines: &State<MachinesState>,
) {
    state.0.lock().unwrap().follow_setup(
        &templates.0.lock().unwrap().value,
        &machines.0.lock().unwrap().value,
    );
}

/// A change to the templates, then the open project follows.
fn change_templates<R>(
    state: &State<AppState>,
    templates: &State<TemplatesState>,
    machines: &State<MachinesState>,
    change: impl FnOnce(&mut ProjectTemplates) -> Result<R, String>,
) -> Result<R, String> {
    let result = change_settings(&templates.0, change)?;
    follow_setup(state, templates, machines);
    Ok(result)
}

/// A change to the machines, then the open project follows.
fn change_machines<R>(
    state: &State<AppState>,
    templates: &State<TemplatesState>,
    machines: &State<MachinesState>,
    change: impl FnOnce(&mut Machines) -> Result<R, String>,
) -> Result<R, String> {
    let result = change_settings(&machines.0, change)?;
    follow_setup(state, templates, machines);
    Ok(result)
}

#[tauri::command]
fn add_template(templates: State<TemplatesState>) -> Result<String, String> {
    change_settings(&templates.0, |t| Ok(t.add_template()))
}

/// Adds an editable copy of a template. Returns its name.
#[tauri::command]
fn new_template_from(name: String, templates: State<TemplatesState>) -> Result<String, String> {
    change_settings(&templates.0, |t| t.new_from(&name))
}

/// Renames a template (a project using it follows). Returns the new name.
#[tauri::command]
fn rename_template(
    old: String,
    new: String,
    state: State<AppState>,
    templates: State<TemplatesState>,
) -> Result<String, String> {
    let new = change_settings(&templates.0, |t| t.rename_template(&old, &new))?;
    state.0.lock().unwrap().template_renamed(&old, &new);
    Ok(new)
}

#[tauri::command]
fn delete_template(name: String, templates: State<TemplatesState>) -> Result<(), String> {
    change_settings(&templates.0, |t| t.delete_template(&name))
}

#[tauri::command]
fn set_template_for_new_projects(
    name: String,
    templates: State<TemplatesState>,
) -> Result<(), String> {
    change_settings(&templates.0, |t| t.set_for_new_projects(&name))
}

#[tauri::command]
fn set_template_grid_spacing(
    name: String,
    mm: f64,
    state: State<AppState>,
    templates: State<TemplatesState>,
    machines: State<MachinesState>,
) -> Result<(), String> {
    change_templates(&state, &templates, &machines, |t| {
        t.set_grid_spacing(&name, mm)
    })
}

#[tauri::command]
fn set_template_major_grid_spacing(
    name: String,
    mm: f64,
    state: State<AppState>,
    templates: State<TemplatesState>,
    machines: State<MachinesState>,
) -> Result<(), String> {
    change_templates(&state, &templates, &machines, |t| {
        t.set_major_grid_spacing(&name, mm)
    })
}

#[tauri::command]
fn set_template_grid_area(
    name: String,
    area: GridArea,
    state: State<AppState>,
    templates: State<TemplatesState>,
    machines: State<MachinesState>,
) -> Result<(), String> {
    change_templates(&state, &templates, &machines, |t| {
        t.set_grid_area(&name, area)
    })
}

#[tauri::command]
fn set_template_snap(
    name: String,
    snap: SnapOptions,
    state: State<AppState>,
    templates: State<TemplatesState>,
    machines: State<MachinesState>,
) -> Result<(), String> {
    change_templates(&state, &templates, &machines, |t| t.set_snap(&name, snap))
}

/// Copies a template's values into the open project. Not an undo step.
#[tauri::command]
fn apply_project_template(
    name: String,
    state: State<AppState>,
    templates: State<TemplatesState>,
) -> Result<(), String> {
    let templates = templates.0.lock().unwrap();
    let template = templates
        .value
        .template(&name)
        .ok_or_else(|| format!("There is no project template called \"{name}\"."))?;
    state.0.lock().unwrap().apply_template(template);
    Ok(())
}

/// Puts one of the project's template settings back to the template's value.
#[tauri::command]
fn reset_project_template_setting(
    setting: TemplateSetting,
    state: State<AppState>,
    templates: State<TemplatesState>,
) -> Result<(), String> {
    let mut project = state.0.lock().unwrap();
    let templates = templates.0.lock().unwrap();
    let name = project.document().template.clone();
    let template = templates
        .value
        .template(&name)
        .ok_or_else(|| format!("The template \"{name}\" isn't on this computer."))?;
    project.reset_template_setting(template, setting);
    Ok(())
}

#[tauri::command]
fn add_machine(machines: State<MachinesState>) -> Result<String, String> {
    change_settings(&machines.0, |m| Ok(m.add_machine()))
}

/// Adds an editable copy of a machine. Returns its name.
#[tauri::command]
fn new_machine_from(name: String, machines: State<MachinesState>) -> Result<String, String> {
    change_settings(&machines.0, |m| m.new_from(&name))
}

/// Renames a machine (a project using it follows). Returns the new name.
#[tauri::command]
fn rename_machine(
    old: String,
    new: String,
    state: State<AppState>,
    machines: State<MachinesState>,
) -> Result<String, String> {
    let new = change_settings(&machines.0, |m| m.rename_machine(&old, &new))?;
    state.0.lock().unwrap().machine_renamed(&old, &new);
    Ok(new)
}

#[tauri::command]
fn delete_machine(name: String, machines: State<MachinesState>) -> Result<(), String> {
    change_settings(&machines.0, |m| m.delete_machine(&name))
}

#[tauri::command]
fn set_machine_for_new_projects(
    name: String,
    machines: State<MachinesState>,
) -> Result<(), String> {
    change_settings(&machines.0, |m| m.set_for_new_projects(&name))
}

/// Sets every setting of a machine in the Setup tab.
#[tauri::command]
fn set_machine_settings(
    name: String,
    settings: MachineSettings,
    state: State<AppState>,
    templates: State<TemplatesState>,
    machines: State<MachinesState>,
) -> Result<(), String> {
    change_machines(&state, &templates, &machines, |m| {
        m.set_settings(&name, &settings)
    })
}

/// Copies a machine's settings into the open project. Not an undo step.
#[tauri::command]
fn set_project_machine(
    name: String,
    state: State<AppState>,
    machines: State<MachinesState>,
) -> Result<(), String> {
    let machines = machines.0.lock().unwrap();
    let machine = machines
        .value
        .machine(&name)
        .ok_or_else(|| format!("There is no machine called \"{name}\"."))?;
    state.0.lock().unwrap().apply_machine(machine);
    Ok(())
}

/// Sets the project's own machine settings. Not an undo step.
#[tauri::command]
fn set_project_machine_settings(
    settings: MachineSettings,
    state: State<AppState>,
) -> Result<(), String> {
    state.0.lock().unwrap().set_machine_settings(&settings)
}

/// Puts one of the project's machine settings back to the machine's value.
#[tauri::command]
fn reset_project_machine_setting(
    setting: MachineSetting,
    state: State<AppState>,
    machines: State<MachinesState>,
) -> Result<(), String> {
    let mut project = state.0.lock().unwrap();
    let machines = machines.0.lock().unwrap();
    let name = project.document().machine.clone();
    let machine = machines
        .value
        .machine(&name)
        .ok_or_else(|| format!("The machine \"{name}\" isn't on this computer."))?;
    project.reset_machine_setting(machine, setting);
    Ok(())
}

/// After opening: the project's template and machine whose values in Setup differ from the
/// project's (see `Project::setup_mismatches`).
#[tauri::command]
fn setup_mismatches(
    state: State<AppState>,
    templates: State<TemplatesState>,
    machines: State<MachinesState>,
) -> Vec<String> {
    state.0.lock().unwrap().setup_mismatches(
        &templates.0.lock().unwrap().value,
        &machines.0.lock().unwrap().value,
    )
}

/// The answer to the mismatch question: keep the project's values (true) or use Setup's.
#[tauri::command]
fn resolve_setup_mismatches(
    keep: bool,
    state: State<AppState>,
    templates: State<TemplatesState>,
    machines: State<MachinesState>,
) {
    if keep {
        state.0.lock().unwrap().keep_project_values(
            &templates.0.lock().unwrap().value,
            &machines.0.lock().unwrap().value,
        );
    } else {
        follow_setup(&state, &templates, &machines);
    }
}

// --- Tooling ------------------------------------------------------------------------------------

/// Problems found loading the settings files (tool list, machine list), once each.
#[tauri::command]
fn settings_problems(
    tooling: State<ToolingState>,
    machines: State<MachinesState>,
    templates: State<TemplatesState>,
) -> Vec<String> {
    let mut problems: Vec<String> = tooling
        .0
        .lock()
        .unwrap()
        .problem
        .take()
        .into_iter()
        .collect();
    problems.extend(machines.0.lock().unwrap().problem.take());
    problems.extend(templates.0.lock().unwrap().problem.take());
    problems
}

/// Starting values for the Add tool form.
#[tauri::command]
fn new_tool() -> Tool {
    Tool::new_default()
}

/// What the tool form shows while it is being filled in.
#[derive(Serialize)]
struct ToolPreview {
    /// The name the tool will get (the automatic one if it uses it).
    name: String,
    /// The side view, or None while the values aren't valid.
    drawing: Option<ToolDrawing>,
}

#[tauri::command]
fn tool_preview(tool: Tool) -> ToolPreview {
    let tool = tool.named();
    ToolPreview {
        drawing: tool.drawing(),
        name: tool.name,
    }
}

#[tauri::command]
fn add_tool(tool: Tool, tooling: State<ToolingState>) -> Result<u32, String> {
    change_tooling(&tooling, |t| t.add_tool(tool))
}

#[tauri::command]
fn set_tool(tool: Tool, tooling: State<ToolingState>) -> Result<(), String> {
    change_tooling(&tooling, |t| t.set_tool(tool))
}

#[tauri::command]
fn delete_tool(id: u32, tooling: State<ToolingState>) -> Result<bool, String> {
    change_tooling(&tooling, |t| Ok(t.delete_tool(id)))
}

#[tauri::command]
fn add_library(tooling: State<ToolingState>) -> Result<String, String> {
    change_tooling(&tooling, |t| Ok(t.add_library()))
}

/// Renames a library. Returns the new name. Like everything in the Setup tab, it doesn't change
/// the open project: a project using the old name shows it as not on this computer until
/// another library is chosen in its Machining properties.
#[tauri::command]
fn rename_library(
    old: String,
    new: String,
    tooling: State<ToolingState>,
) -> Result<String, String> {
    change_tooling(&tooling, |t| t.rename_library(&old, &new))
}

#[tauri::command]
fn delete_library(name: String, tooling: State<ToolingState>) -> Result<bool, String> {
    change_tooling(&tooling, |t| Ok(t.delete_library(&name)))
}

#[tauri::command]
fn add_library_tool(
    library: String,
    tool_id: u32,
    number: u32,
    tooling: State<ToolingState>,
) -> Result<(), String> {
    change_tooling(&tooling, |t| t.add_library_tool(&library, tool_id, number))
}

#[tauri::command]
fn set_library_tool_number(
    library: String,
    old: u32,
    new: u32,
    tooling: State<ToolingState>,
) -> Result<(), String> {
    change_tooling(&tooling, |t| t.set_library_tool_number(&library, old, new))
}

#[tauri::command]
fn remove_library_tool(
    library: String,
    number: u32,
    tooling: State<ToolingState>,
) -> Result<bool, String> {
    change_tooling(&tooling, |t| Ok(t.remove_library_tool(&library, number)))
}

#[tauri::command]
fn set_active_library(name: Option<String>, state: State<AppState>) {
    state.0.lock().unwrap().set_active_library(name);
}

/// Tool `number` from the project's active library on this computer, as a copy for the project.
fn library_pick(
    project: &Project,
    tooling: &State<ToolingState>,
    number: u32,
) -> Result<PickedTool, String> {
    let library = project
        .document()
        .active_library
        .clone()
        .ok_or("Choose a tool library first (Machining properties).")?;
    let tooling = tooling.0.lock().unwrap();
    let tool = tooling
        .value
        .library_tool(&library, number)
        .ok_or_else(|| {
            format!("There is no tool {number} in the tool library \"{library}\" on this computer.")
        })?;
    Ok(PickedTool {
        number,
        tool: tool.clone(),
    })
}

/// Sets the Machining tool to tool `number` of the active library (None = none).
#[tauri::command]
fn set_machining_tool(
    number: Option<u32>,
    state: State<AppState>,
    tooling: State<ToolingState>,
) -> Result<(), String> {
    let mut project = state.0.lock().unwrap();
    let pick = number
        .map(|n| library_pick(&project, &tooling, n))
        .transpose()?;
    project.set_machining_tool(pick)
}

/// Sets part `part`'s own tool to tool `number` of the active library (None = use Machining's).
#[tauri::command]
fn set_part_tool(
    part: usize,
    number: Option<u32>,
    state: State<AppState>,
    tooling: State<ToolingState>,
) -> Result<(), String> {
    let mut project = state.0.lock().unwrap();
    let pick = number
        .map(|n| library_pick(&project, &tooling, n))
        .transpose()?;
    project.set_part_tool(part, pick)
}

/// How an operation gets its tool, as the interface sends it.
#[derive(serde::Deserialize)]
enum OperationTool {
    /// The part's tool, or Machining's.
    Inherit,
    /// Its own tool with values set in the operation.
    Custom,
    /// Its own tool: this number from the active library.
    Library(u32),
}

#[tauri::command]
fn set_operation_tool(
    id: u32,
    tool: OperationTool,
    state: State<AppState>,
    tooling: State<ToolingState>,
) -> Result<(), String> {
    let mut project = state.0.lock().unwrap();
    tell_library_numbers(&mut project, &tooling);
    let setting = match tool {
        OperationTool::Inherit => ToolSetting::Inherit,
        OperationTool::Custom => ToolSetting::Custom,
        OperationTool::Library(n) => ToolSetting::Library(library_pick(&project, &tooling, n)?),
    };
    project.set_operation_tool(id, setting)
}

// --- Project files ------------------------------------------------------------------------------

#[tauri::command]
fn new_project(
    state: State<AppState>,
    file: State<OpenFile>,
    templates: State<TemplatesState>,
    machines: State<MachinesState>,
) {
    let templates = templates.0.lock().unwrap();
    let machines = machines.0.lock().unwrap();
    state
        .0
        .lock()
        .unwrap()
        .new_project(templates.value.for_new(), machines.value.for_new());
    *file.0.lock().unwrap() = None;
}

#[tauri::command]
fn open_project(path: String, state: State<AppState>, file: State<OpenFile>) -> Result<(), String> {
    let path = PathBuf::from(path);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    state.0.lock().unwrap().open(&text, &project_name(&path))?;
    *file.0.lock().unwrap() = Some(path);
    Ok(())
}

/// Saves the project to `path` (adding the .whamcam extension if missing), or to the file it
/// was last saved to or opened from if `path` is None.
#[tauri::command]
fn save_project(
    path: Option<String>,
    state: State<AppState>,
    file: State<OpenFile>,
) -> Result<(), String> {
    let path = match path {
        Some(p) => with_project_extension(PathBuf::from(p)),
        None => file
            .0
            .lock()
            .unwrap()
            .clone()
            .ok_or("The project hasn't been saved yet.")?,
    };
    let mut project = state.0.lock().unwrap();
    let text = project.save_text(&project_name(&path));
    write_safely(&path, &text).map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    project.mark_saved();
    *file.0.lock().unwrap() = Some(path);
    Ok(())
}

fn with_project_extension(path: PathBuf) -> PathBuf {
    let ext = wham_engine::project_file::EXTENSION;
    match path.extension() {
        Some(e) if e.eq_ignore_ascii_case(ext) => path,
        _ => {
            let mut name = path.into_os_string();
            name.push(format!(".{ext}"));
            PathBuf::from(name)
        }
    }
}

/// Writes to a temporary file next to `path`, then renames it over `path`, so a failed save
/// never leaves a half-written project behind.
fn write_safely(path: &Path, text: &str) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".saving");
    let tmp = PathBuf::from(tmp);
    let result = std::fs::write(&tmp, text).and_then(|()| std::fs::rename(&tmp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[tauri::command]
fn dxf_info(path: String) -> Result<DxfInfo, String> {
    Project::dxf_info(&read_text(&path)?).map_err(|e| e.to_string())
}

#[tauri::command]
fn svg_info(path: String) -> Result<SvgInfo, String> {
    Project::svg_info(&read_text(&path)?).map_err(|e| e.to_string())
}

#[tauri::command]
fn import_svg(
    path: String,
    mm_per_unit: f64,
    curve_tolerance: f64,
    default_layer: String,
    state: State<AppState>,
) -> Result<ImportReport, String> {
    let text = read_text(&path)?;
    state
        .0
        .lock()
        .unwrap()
        .import_svg(&text, mm_per_unit, curve_tolerance, &default_layer)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn import_dxf(
    path: String,
    scale: f64,
    curve_tolerance: f64,
    state: State<AppState>,
) -> Result<ImportReport, String> {
    let text = read_text(&path)?;
    state
        .0
        .lock()
        .unwrap()
        .import_dxf(&text, scale, curve_tolerance)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_objects(ids: Vec<u32>, state: State<AppState>) -> usize {
    state.0.lock().unwrap().delete(&ids)
}

#[tauri::command]
fn add_layer(state: State<AppState>) -> String {
    state.0.lock().unwrap().add_layer()
}

#[tauri::command]
fn delete_layer(name: String, state: State<AppState>) -> bool {
    state.0.lock().unwrap().delete_layer(&name)
}

#[tauri::command]
fn rename_layer(old: String, new: String, state: State<AppState>) -> Result<(), String> {
    state.0.lock().unwrap().rename_layer(&old, &new)
}

#[tauri::command]
fn set_layer_color(name: String, color: String, state: State<AppState>) -> Result<(), String> {
    state.0.lock().unwrap().set_layer_color(&name, &color)
}

/// The zoom in mm per screen pixel, so dimensions keep their size on screen.
#[tauri::command]
fn set_view_scale(mm_per_px: f64, state: State<AppState>) -> Vec<DisplayShape> {
    state.0.lock().unwrap().set_view_scale(mm_per_px)
}

/// Shows or hides every dimension (a view setting: not saved, not an undo step).
#[tauri::command]
fn set_dimensions_visible(visible: bool, state: State<AppState>) {
    state.0.lock().unwrap().set_dimensions_visible(visible);
}

/// The dimension under (x, y), for dragging it.
#[tauri::command]
fn dimension_at(x: f64, y: f64, tolerance: f64, state: State<AppState>) -> Option<u32> {
    state.0.lock().unwrap().dimension_at(x, y, tolerance)
}

/// A dimension moved to (x, y), as it would be drawn while dragging it.
#[tauri::command]
fn dimension_drag_preview(id: u32, x: f64, y: f64, state: State<AppState>) -> Option<DisplayShape> {
    state.0.lock().unwrap().dimension_drag_preview(id, x, y)
}

/// Moves where a dimension sits, as one undo step.
#[tauri::command]
fn reposition_dimension(id: u32, x: f64, y: f64, state: State<AppState>) -> Result<(), String> {
    state.0.lock().unwrap().reposition_dimension(id, x, y)
}

/// Shows or hides a layer (a view setting: not saved, not an undo step).
#[tauri::command]
fn set_layer_visible(name: String, visible: bool, state: State<AppState>) -> Result<(), String> {
    state.0.lock().unwrap().set_layer_visible(&name, visible)
}

#[tauri::command]
fn undo(state: State<AppState>) -> bool {
    state.0.lock().unwrap().undo()
}

#[tauri::command]
fn redo(state: State<AppState>) -> bool {
    state.0.lock().unwrap().redo()
}

#[tauri::command]
fn pick(x: f64, y: f64, tolerance: f64, state: State<AppState>) -> Option<u32> {
    state.0.lock().unwrap().pick(x, y, tolerance)
}

#[tauri::command]
fn pick_box(x0: f64, y0: f64, x1: f64, y1: f64, mode: BoxMode, state: State<AppState>) -> Vec<u32> {
    state.0.lock().unwrap().pick_box(x0, y0, x1, y1, mode)
}

#[tauri::command]
fn snap(
    x: f64,
    y: f64,
    tolerance: f64,
    options: SnapOptions,
    extra: Vec<Point>,
    state: State<AppState>,
) -> Snap {
    state
        .0
        .lock()
        .unwrap()
        .snap(x, y, tolerance, options, &extra)
}

#[tauri::command]
fn preview_shape(
    draft: Draft,
    cursor: Option<Point>,
    state: State<AppState>,
) -> Option<DisplayShape> {
    state.0.lock().unwrap().preview(&draft, cursor)
}

/// What a Dimension tool click at (x, y) is: a snapped point or a picked object, and whether it
/// completes the dimension.
#[tauri::command]
fn dimension_click(
    draft: Draft,
    x: f64,
    y: f64,
    tolerance: f64,
    options: SnapOptions,
    state: State<AppState>,
) -> DimensionClick {
    state
        .0
        .lock()
        .unwrap()
        .dimension_click(&draft, x, y, tolerance, options)
}

#[tauri::command]
fn add_draft(layer: String, draft: Draft, state: State<AppState>) -> Result<u32, String> {
    state.0.lock().unwrap().add_draft(&layer, &draft)
}

#[tauri::command]
fn set_shape(id: u32, shape: Shape, state: State<AppState>) -> Result<(), String> {
    state.0.lock().unwrap().set_shape(id, shape)
}

#[tauri::command]
fn set_grid_spacing(mm: f64, state: State<AppState>) -> Result<(), String> {
    state.0.lock().unwrap().set_grid_spacing(mm)
}

#[tauri::command]
fn set_major_grid_spacing(mm: f64, state: State<AppState>) -> Result<(), String> {
    state.0.lock().unwrap().set_major_grid_spacing(mm)
}

#[tauri::command]
fn set_grid_area(area: GridArea, state: State<AppState>) -> Result<(), String> {
    state.0.lock().unwrap().set_grid_area(area)
}

#[tauri::command]
fn set_snap(snap: SnapOptions, state: State<AppState>) {
    state.0.lock().unwrap().set_snap(snap);
}

#[tauri::command]
fn transform(ids: Vec<u32>, op: TransformOp, state: State<AppState>) -> Result<(), String> {
    state.0.lock().unwrap().transform(&ids, op)
}

#[tauri::command]
fn join(ids: Vec<u32>, tolerance: f64, state: State<AppState>) -> Result<JoinReport, String> {
    state.0.lock().unwrap().join(&ids, tolerance)
}

#[tauri::command]
fn explode(ids: Vec<u32>, state: State<AppState>) -> Vec<u32> {
    state.0.lock().unwrap().explode(&ids)
}

#[tauri::command]
fn close_polylines(ids: Vec<u32>, state: State<AppState>) -> usize {
    state.0.lock().unwrap().close_polylines(&ids)
}

#[tauri::command]
fn break_at_intersections(ids: Vec<u32>, state: State<AppState>) -> BreakReport {
    state.0.lock().unwrap().break_at_intersections(&ids)
}

// --- Operations ---------------------------------------------------------------------------------

#[tauri::command]
fn add_pocket(shape_ids: Vec<u32>, state: State<AppState>) -> Result<u32, String> {
    state.0.lock().unwrap().add_pocket(&shape_ids)
}

#[tauri::command]
fn add_drill(shape_ids: Vec<u32>, state: State<AppState>) -> Result<u32, String> {
    state.0.lock().unwrap().add_drill(&shape_ids)
}

#[tauri::command]
fn add_profile(shape_ids: Vec<u32>, state: State<AppState>) -> Result<u32, String> {
    state.0.lock().unwrap().add_profile(&shape_ids)
}

#[tauri::command]
fn set_operation(
    op: Operation,
    state: State<AppState>,
    tooling: State<ToolingState>,
) -> Result<(), String> {
    let mut project = state.0.lock().unwrap();
    tell_library_numbers(&mut project, &tooling);
    project.set_operation(op)
}

/// Tells the project the tools of its library on this computer, so tool numbers can be checked
/// against them (empty if no library is chosen; None if it isn't on this computer).
fn tell_library_numbers(project: &mut Project, tooling: &State<ToolingState>) {
    let tools = match project.document().active_library.as_deref() {
        None => Some(Vec::new()),
        Some(name) => {
            let tooling = &tooling.0.lock().unwrap().value;
            tooling.library(name).map(|lib| {
                lib.tools
                    .iter()
                    .filter_map(|e| {
                        Some(PickedTool {
                            number: e.number,
                            tool: tooling.tool(e.tool_id)?.clone(),
                        })
                    })
                    .collect()
            })
        }
    };
    project.set_library_tools(tools);
}

#[tauri::command]
fn set_operation_shapes(
    id: u32,
    shape_ids: Vec<u32>,
    state: State<AppState>,
) -> Result<(), String> {
    state.0.lock().unwrap().set_operation_shapes(id, &shape_ids)
}

#[tauri::command]
fn delete_operation(id: u32, state: State<AppState>) -> bool {
    state.0.lock().unwrap().delete_operation(id)
}

// --- G-code -------------------------------------------------------------------------------------

/// Checks the G-code can be written (toolpaths generated and up to date, read-back check passed).
/// Returns the suggested file name, or the reason it can't be written.
#[tauri::command]
fn check_gcode(
    date: String,
    state: State<AppState>,
    tooling: State<ToolingState>,
) -> Result<String, String> {
    let mut project = state.0.lock().unwrap();
    tell_library_numbers(&mut project, &tooling);
    project.gcode(&date)?;
    Ok(format!("{}.nc", project.document().name))
}

/// Writes the G-code to `path` (adding `.nc` if it has no extension). Returns the file name.
#[tauri::command]
fn save_gcode(path: String, date: String, state: State<AppState>) -> Result<String, String> {
    let program = state.0.lock().unwrap().gcode(&date)?;
    let mut path = PathBuf::from(path);
    if path.extension().is_none() {
        path.set_extension("nc");
    }
    write_safely(&path, &program.text())
        .map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    Ok(path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default())
}

#[tauri::command]
fn generate_toolpaths(
    ids: Vec<u32>,
    state: State<AppState>,
    tooling: State<ToolingState>,
) -> GenerateReport {
    let mut project = state.0.lock().unwrap();
    tell_library_numbers(&mut project, &tooling);
    project.generate(&ids)
}

#[tauri::command]
fn cut_widths(state: State<AppState>) -> Vec<CutWidthStep> {
    state.0.lock().unwrap().cut_widths()
}

#[tauri::command]
fn toolpath_playback(state: State<AppState>) -> Playback {
    state.0.lock().unwrap().playback()
}

#[tauri::command]
fn transform_preview(ids: Vec<u32>, op: TransformOp, state: State<AppState>) -> Vec<DisplayShape> {
    state.0.lock().unwrap().transform_preview(&ids, op)
}

#[tauri::command]
fn selection_centre(ids: Vec<u32>, state: State<AppState>) -> Option<Point> {
    state.0.lock().unwrap().selection_centre(&ids)
}

#[tauri::command]
fn copy(ids: Vec<u32>, state: State<AppState>) -> usize {
    state.0.lock().unwrap().copy(&ids)
}

#[tauri::command]
fn cut(ids: Vec<u32>, state: State<AppState>) -> usize {
    state.0.lock().unwrap().cut(&ids)
}

#[tauri::command]
fn has_clipboard(state: State<AppState>) -> bool {
    state.0.lock().unwrap().has_clipboard()
}

#[tauri::command]
fn paste_preview(at: Point, state: State<AppState>) -> Vec<DisplayShape> {
    state.0.lock().unwrap().paste_preview(at)
}

#[tauri::command]
fn paste(at: Point, state: State<AppState>) -> Vec<u32> {
    state.0.lock().unwrap().paste(at)
}

#[tauri::command]
fn all_ids(state: State<AppState>) -> Vec<u32> {
    state.0.lock().unwrap().all_ids()
}

#[tauri::command]
fn ids_on_layer(name: String, state: State<AppState>) -> Vec<u32> {
    state.0.lock().unwrap().ids_on_layer(&name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState(Mutex::new(Project::new())))
        .manage(OpenFile(Mutex::new(None)))
        .setup(|app| {
            let dir = app.path().app_config_dir().ok();
            app.manage(ToolingState(Mutex::new(load_settings(
                dir.as_deref(),
                "tooling.json",
                "tool list",
                wham_engine::tooling::from_text,
                wham_engine::tooling::to_text,
            ))));
            let machines = load_settings(
                dir.as_deref(),
                "machines.json",
                "machine list",
                wham_engine::machine::from_text,
                wham_engine::machine::to_text,
            );
            let templates = load_settings(
                dir.as_deref(),
                "project_templates.json",
                "project templates",
                wham_engine::project_template::from_text,
                wham_engine::project_template::to_text,
            );
            // The project shown at startup starts from the template and machine for new
            // projects.
            app.state::<AppState>()
                .0
                .lock()
                .unwrap()
                .new_project(templates.value.for_new(), machines.value.for_new());
            app.manage(TemplatesState(Mutex::new(templates)));
            app.manage(MachinesState(Mutex::new(machines)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_view,
            settings_problems,
            add_machine,
            rename_machine,
            delete_machine,
            new_machine_from,
            set_machine_for_new_projects,
            set_machine_settings,
            set_project_machine,
            set_project_machine_settings,
            reset_project_machine_setting,
            reset_project_template_setting,
            new_template_from,
            setup_mismatches,
            resolve_setup_mismatches,
            add_template,
            rename_template,
            delete_template,
            set_template_for_new_projects,
            set_template_grid_spacing,
            set_template_major_grid_spacing,
            set_template_grid_area,
            set_template_snap,
            apply_project_template,
            check_gcode,
            save_gcode,
            new_tool,
            tool_preview,
            add_tool,
            set_tool,
            delete_tool,
            add_library,
            rename_library,
            delete_library,
            add_library_tool,
            set_library_tool_number,
            remove_library_tool,
            set_active_library,
            set_operation_tool,
            set_machining_tool,
            set_part_tool,
            new_project,
            open_project,
            save_project,
            dxf_info,
            import_dxf,
            svg_info,
            import_svg,
            delete_objects,
            add_layer,
            delete_layer,
            rename_layer,
            set_layer_color,
            set_layer_visible,
            set_view_scale,
            set_dimensions_visible,
            dimension_at,
            dimension_drag_preview,
            reposition_dimension,
            undo,
            redo,
            pick,
            pick_box,
            snap,
            preview_shape,
            dimension_click,
            add_draft,
            set_shape,
            set_grid_spacing,
            set_major_grid_spacing,
            set_grid_area,
            set_snap,
            transform,
            transform_preview,
            selection_centre,
            copy,
            cut,
            has_clipboard,
            paste_preview,
            paste,
            all_ids,
            ids_on_layer,
            join,
            explode,
            close_polylines,
            break_at_intersections,
            add_pocket,
            add_profile,
            add_drill,
            set_operation,
            set_operation_shapes,
            delete_operation,
            generate_toolpaths,
            toolpath_playback,
            cut_widths
        ])
        .run(tauri::generate_context!())
        .expect("error while running WHAM_CAM");
}
