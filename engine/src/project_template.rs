//! Project templates (spec 5.9): saved sets of a project's interface settings (grid spacing,
//! grid area, snaps). Like the machines, the templates belong to the computer: a project keeps a
//! copy of its template's values, which follow the template except the ones changed in the
//! project. Nothing here affects toolpaths or G-code. The engine turns the list into text and
//! back; the app reads and writes the file.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::document::{
    DEFAULT_GRID_SPACING, DEFAULT_MAJOR_GRID_SPACING, GridArea, check_grid_spacing,
    check_major_grid_spacing,
};
use crate::draw::SnapOptions;

/// The built-in template: always in the list, can't be renamed, edited or deleted.
pub const DEFAULT_TEMPLATE: &str = "Default";

/// One template setting, to say which ones a project has changed for itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TemplateSetting {
    GridSpacing,
    MajorGridSpacing,
    GridArea,
    SnapToGrid,
    SnapToObjects,
}

impl TemplateSetting {
    pub const ALL: [TemplateSetting; 5] = [
        TemplateSetting::GridSpacing,
        TemplateSetting::MajorGridSpacing,
        TemplateSetting::GridArea,
        TemplateSetting::SnapToGrid,
        TemplateSetting::SnapToObjects,
    ];
}

/// The error for changing the Default template.
fn read_only() -> String {
    format!(
        "\"{DEFAULT_TEMPLATE}\" is built in and can't be changed. Use New from to make an editable copy."
    )
}

/// One project template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectTemplate {
    /// Unique among the templates.
    pub name: String,
    pub grid_spacing: f64,
    /// Spacing of the heavier grid lines.
    pub major_grid_spacing: f64,
    pub grid_area: GridArea,
    pub snap: SnapOptions,
}

impl ProjectTemplate {
    /// A template with the built-in values: grid 10 mm, the default grid area, both snaps on.
    pub fn built_in(name: &str) -> Self {
        Self {
            name: name.to_string(),
            grid_spacing: DEFAULT_GRID_SPACING,
            major_grid_spacing: DEFAULT_MAJOR_GRID_SPACING,
            grid_area: GridArea::default(),
            snap: SnapOptions::default(),
        }
    }
}

/// The project templates of this computer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectTemplates {
    pub templates: Vec<ProjectTemplate>,
    /// The template File ▸ New (and startup) uses. Always one of `templates`.
    pub for_new_projects: String,
}

impl Default for ProjectTemplates {
    /// Just the built-in Default template, used for new projects.
    fn default() -> Self {
        Self {
            templates: vec![ProjectTemplate::built_in(DEFAULT_TEMPLATE)],
            for_new_projects: DEFAULT_TEMPLATE.to_string(),
        }
    }
}

impl ProjectTemplates {
    pub fn template(&self, name: &str) -> Option<&ProjectTemplate> {
        self.templates.iter().find(|t| t.name == name)
    }

    /// A template other than Default, to change.
    fn template_mut(&mut self, name: &str) -> Result<&mut ProjectTemplate, String> {
        if name == DEFAULT_TEMPLATE {
            return Err(read_only());
        }
        self.templates
            .iter_mut()
            .find(|t| t.name == name)
            .ok_or_else(|| format!("There is no project template called \"{name}\"."))
    }

    /// The template new projects start from.
    pub fn for_new(&self) -> &ProjectTemplate {
        self.template(&self.for_new_projects)
            .or(self.templates.first())
            .expect("there is always at least one template")
    }

    /// The first free name of the form "<base> 1", "<base> 2", ... (or `first` if free).
    fn free_name(&self, first: Option<&str>, base: &str) -> String {
        first
            .filter(|n| self.template(n).is_none())
            .map(str::to_string)
            .unwrap_or_else(|| {
                (1..)
                    .map(|n| format!("{base} {n}"))
                    .find(|n| self.template(n).is_none())
                    .expect("there is always a free name")
            })
    }

    /// Adds a template called "Template 1", "Template 2", ... with the built-in values.
    /// Returns its name.
    pub fn add_template(&mut self) -> String {
        let name = self.free_name(None, "Template");
        self.templates.push(ProjectTemplate::built_in(&name));
        name
    }

    /// Adds an editable copy of template `from`, called "<from> copy" (then "<from> copy 2",
    /// ...). Returns its name.
    pub fn new_from(&mut self, from: &str) -> Result<String, String> {
        let mut copy = self
            .template(from)
            .ok_or_else(|| format!("There is no project template called \"{from}\"."))?
            .clone();
        let base = format!("{from} copy");
        copy.name = self.free_name(Some(&base), &base);
        let name = copy.name.clone();
        self.templates.push(copy);
        Ok(name)
    }

    /// Renames a template. Returns the new name as stored (trimmed). Default can't be renamed,
    /// and no other template can be called "Default" (in any capitals).
    pub fn rename_template(&mut self, old: &str, new: &str) -> Result<String, String> {
        let new = new.trim();
        if new.is_empty() {
            return Err("A project template needs a name.".into());
        }
        if new.eq_ignore_ascii_case(DEFAULT_TEMPLATE) && old != DEFAULT_TEMPLATE {
            return Err(format!(
                "\"{new}\" can't be used: it is the built-in template's name."
            ));
        }
        if new != old && self.template(new).is_some() {
            return Err(format!(
                "There is already a project template called \"{new}\"."
            ));
        }
        self.template_mut(old)?.name = new.to_string();
        if self.for_new_projects == old {
            self.for_new_projects = new.to_string();
        }
        Ok(new.to_string())
    }

    /// Deletes a template. Default can't be deleted. If it was used for new projects, Default is
    /// used instead.
    pub fn delete_template(&mut self, name: &str) -> Result<(), String> {
        self.template_mut(name)?;
        self.templates.retain(|t| t.name != name);
        if self.for_new_projects == name {
            self.for_new_projects = DEFAULT_TEMPLATE.to_string();
        }
        Ok(())
    }

    /// Makes File ▸ New start from this template.
    pub fn set_for_new_projects(&mut self, name: &str) -> Result<(), String> {
        if self.template(name).is_none() {
            return Err(format!("There is no project template called \"{name}\"."));
        }
        self.for_new_projects = name.to_string();
        Ok(())
    }

    pub fn set_grid_spacing(&mut self, name: &str, mm: f64) -> Result<(), String> {
        check_grid_spacing(mm)?;
        self.template_mut(name)?.grid_spacing = mm;
        Ok(())
    }

    pub fn set_major_grid_spacing(&mut self, name: &str, mm: f64) -> Result<(), String> {
        check_major_grid_spacing(mm)?;
        self.template_mut(name)?.major_grid_spacing = mm;
        Ok(())
    }

    pub fn set_grid_area(&mut self, name: &str, area: GridArea) -> Result<(), String> {
        area.validate()?;
        self.template_mut(name)?.grid_area = area;
        Ok(())
    }

    pub fn set_snap(&mut self, name: &str, snap: SnapOptions) -> Result<(), String> {
        self.template_mut(name)?.snap = snap;
        Ok(())
    }
}

// --- Project templates file ---------------------------------------------------------------------

/// The value of the "format" field, so other JSON files are recognised as not ours.
pub const FORMAT_NAME: &str = "WHAM_CAM project templates";

/// The project templates file format version written by this WHAM_CAM.
pub const FORMAT_VERSION: u32 = 1;

#[derive(Serialize)]
struct FileOut<'a> {
    format: &'a str,
    format_version: u32,
    saved_by: String,
    #[serde(flatten)]
    templates: &'a ProjectTemplates,
}

/// The project templates as file text (pretty-printed JSON).
pub fn to_text(templates: &ProjectTemplates) -> String {
    let file = FileOut {
        format: FORMAT_NAME,
        format_version: FORMAT_VERSION,
        saved_by: format!("WHAM_CAM {}", env!("CARGO_PKG_VERSION")),
        templates,
    };
    serde_json::to_string_pretty(&file).expect("templates always convert to JSON") + "\n"
}

/// Reads project templates file text. Errors are plain-English messages saying what is wrong.
pub fn from_text(text: &str) -> Result<ProjectTemplates, String> {
    const NOT_OURS: &str = "This isn't a WHAM_CAM project templates file.";
    let value: Value = serde_json::from_str(text).map_err(|_| NOT_OURS.to_string())?;
    if value.get("format").and_then(Value::as_str) != Some(FORMAT_NAME) {
        return Err(NOT_OURS.into());
    }
    let version = value
        .get("format_version")
        .and_then(Value::as_u64)
        .ok_or("The project templates file has no format version; it may be damaged.")?;
    if version > u64::from(FORMAT_VERSION) {
        return Err(
            "The project templates file was made with a newer WHAM_CAM. Please update WHAM_CAM."
                .into(),
        );
    }
    let mut templates: ProjectTemplates = serde_json::from_value(value)
        .map_err(|e| format!("The project templates file is damaged ({e})."))?;
    // Default is built in: whatever the file says about it, it has the built-in values.
    templates
        .templates
        .retain(|t| !t.name.eq_ignore_ascii_case(DEFAULT_TEMPLATE));
    templates
        .templates
        .insert(0, ProjectTemplate::built_in(DEFAULT_TEMPLATE));
    let damaged = |what: String| Err(format!("The project templates file is damaged: {what}."));
    let mut names = HashSet::new();
    for t in &templates.templates {
        if t.name.trim().is_empty() {
            return damaged("a template has no name".into());
        }
        if !names.insert(t.name.as_str()) {
            return damaged(format!("there are two templates called \"{}\"", t.name));
        }
        if let Err(e) = check_grid_spacing(t.grid_spacing)
            .and(check_major_grid_spacing(t.major_grid_spacing))
            .and(t.grid_area.validate())
        {
            return damaged(format!("\"{}\": {e}", t.name));
        }
    }
    if templates.template(&templates.for_new_projects).is_none() {
        return damaged(format!(
            "the template for new projects, \"{}\", isn't in the list",
            templates.for_new_projects
        ));
    }
    Ok(templates)
}
