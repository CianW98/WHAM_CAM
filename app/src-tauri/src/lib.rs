//! Tauri app layer. Commands here only pass data to and from the engine:
//! no geometry, toolpath or G-code logic belongs in this file.

use wham_engine::{DisplayData, Document};

#[tauri::command]
fn get_document() -> Document {
    wham_engine::demo_document()
}

#[tauri::command]
fn get_display_data() -> DisplayData {
    wham_engine::display_data(&wham_engine::demo_document())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_document, get_display_data])
        .run(tauri::generate_context!())
        .expect("error while running WHAM_CAM");
}
