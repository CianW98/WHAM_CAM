//! WHAM_CAM project files (`.whamcam`): the document as readable JSON with a format version.
//! The engine only turns a document into text and back; the app reads and writes the file.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::document::{Document, is_hex_color};
use crate::draw;
use crate::geometry::Shape;

/// The value of the "format" field, so other JSON files are recognised as not ours.
pub const FORMAT_NAME: &str = "WHAM_CAM project";

/// The file format version written by this WHAM_CAM. Bump it when the format changes in a way
/// older versions can't read, and upgrade older files in [`from_text`].
pub const FORMAT_VERSION: u32 = 1;

/// The file extension for project files (without the dot).
pub const EXTENSION: &str = "whamcam";

#[derive(Serialize)]
struct FileOut<'a> {
    format: &'a str,
    format_version: u32,
    saved_by: String,
    document: &'a Document,
}

#[derive(Deserialize)]
struct FileIn {
    document: Document,
}

/// The document as project-file text (pretty-printed JSON).
pub fn to_text(doc: &Document) -> String {
    let file = FileOut {
        format: FORMAT_NAME,
        format_version: FORMAT_VERSION,
        saved_by: format!("WHAM_CAM {}", env!("CARGO_PKG_VERSION")),
        document: doc,
    };
    serde_json::to_string_pretty(&file).expect("a document always converts to JSON") + "\n"
}

/// Reads project-file text. Errors are plain-English messages saying what is wrong.
pub fn from_text(text: &str) -> Result<Document, String> {
    const NOT_OURS: &str = "This isn't a WHAM_CAM project file.";
    let value: Value = serde_json::from_str(text).map_err(|_| NOT_OURS.to_string())?;
    if value.get("format").and_then(Value::as_str) != Some(FORMAT_NAME) {
        return Err(NOT_OURS.into());
    }
    let version = value
        .get("format_version")
        .and_then(Value::as_u64)
        .ok_or("The project file has no format version; it may be damaged.")?;
    if version > u64::from(FORMAT_VERSION) {
        let by = value
            .get("saved_by")
            .and_then(Value::as_str)
            .unwrap_or("a newer WHAM_CAM");
        return Err(format!(
            "This project was made with a newer WHAM_CAM ({by}). Please update WHAM_CAM to open it."
        ));
    }
    let file: FileIn = serde_json::from_value(value)
        .map_err(|e| format!("The project file is damaged and can't be opened ({e})."))?;
    check(&file.document)?;
    Ok(file.document)
}

/// Checks a document read from a file makes sense, so a damaged file can't cause trouble later.
fn check(doc: &Document) -> Result<(), String> {
    let damaged = |what: String| Err(format!("The project file is damaged: {what}."));
    if !(doc.grid_spacing.is_finite() && doc.grid_spacing > 0.0) {
        return damaged("the grid spacing must be a number more than 0".into());
    }
    if doc.layers.is_empty() {
        return damaged("it has no layers".into());
    }
    let mut names = HashSet::new();
    let mut ids = HashSet::new();
    for layer in &doc.layers {
        if layer.name.trim().is_empty() {
            return damaged("a layer has no name".into());
        }
        if !names.insert(layer.name.as_str()) {
            return damaged(format!("there are two layers called \"{}\"", layer.name));
        }
        if !is_hex_color(&layer.color) {
            return damaged(format!(
                "layer \"{}\" has a colour that isn't valid",
                layer.name
            ));
        }
        for obj in &layer.objects {
            if !ids.insert(obj.id) {
                return damaged(format!("two objects have the id {}", obj.id));
            }
            let problem = draw::validate(&obj.shape)
                .err()
                .or_else(|| spline_problem(&obj.shape));
            if let Some(problem) = problem {
                return damaged(format!(
                    "object {} on layer \"{}\": {}",
                    obj.id,
                    layer.name,
                    problem.trim_end_matches('.')
                ));
            }
        }
    }
    let mut op_ids = HashSet::new();
    for op in doc.parts.iter().flat_map(|p| &p.operations) {
        if !op_ids.insert(op.id) {
            return damaged(format!("two operations have the id {}", op.id));
        }
    }
    Ok(())
}

/// Splines need matching counts of control points, knots and weights.
fn spline_problem(shape: &Shape) -> Option<String> {
    let Shape::Spline(s) = shape else {
        return None;
    };
    let n = s.control_points.len();
    let ok = s.degree >= 1
        && n > s.degree
        && s.knots.len() == n + s.degree + 1
        && (s.weights.is_empty() || s.weights.len() == n);
    (!ok).then(|| "a spline's points, knots and weights don't match".to_string())
}
