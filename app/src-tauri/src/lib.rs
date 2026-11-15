//! Tauri app layer. Commands here only pass data to and from the engine, and read files
//! from disk (the engine never touches the file system). No geometry, toolpath or G-code
//! logic belongs in this file.

use std::sync::Mutex;

use serde::Serialize;
use tauri::State;
use wham_engine::{BoxMode, DisplayData, Document, DxfInfo, ImportReport, Project};

/// The open project, shared by all commands.
struct AppState(Mutex<Project>);

/// Everything the interface needs to redraw after a change.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct View {
    document: Document,
    display: DisplayData,
    can_undo: bool,
    can_redo: bool,
}

fn view(p: &Project) -> View {
    View {
        document: p.document().clone(),
        display: p.display().clone(),
        can_undo: p.can_undo(),
        can_redo: p.can_redo(),
    }
}

/// Reads a file as text. Non-UTF-8 bytes (older DXFs use Windows code pages) are replaced;
/// they only ever appear in text such as layer names.
fn read_text(path: &str) -> Result<String, String> {
    std::fs::read(path)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .map_err(|e| format!("Could not read {path}: {e}"))
}

#[tauri::command]
fn get_view(state: State<AppState>) -> View {
    view(&state.0.lock().unwrap())
}

#[tauri::command]
fn dxf_info(path: String) -> Result<DxfInfo, String> {
    Project::dxf_info(&read_text(&path)?).map_err(|e| e.to_string())
}

#[tauri::command]
fn import_dxf(path: String, scale: f64, state: State<AppState>) -> Result<ImportReport, String> {
    let text = read_text(&path)?;
    state
        .0
        .lock()
        .unwrap()
        .import_dxf(&text, scale)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_objects(ids: Vec<u32>, state: State<AppState>) -> usize {
    state.0.lock().unwrap().delete(&ids)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState(Mutex::new(Project::new())))
        .invoke_handler(tauri::generate_handler![
            get_view,
            dxf_info,
            import_dxf,
            delete_objects,
            delete_layer,
            rename_layer,
            set_layer_color,
            undo,
            redo,
            pick,
            pick_box
        ])
        .run(tauri::generate_context!())
        .expect("error while running WHAM_CAM");
}
