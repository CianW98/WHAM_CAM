//! The open project: the document plus undo/redo history and selection hit-testing.
//! The app layer holds one `Project` and calls these methods; the interface never edits
//! the document directly.

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::display::{DisplayData, DisplayShape, display_data};
use crate::document::{Document, DrawingObject};
use crate::dxf::{DxfError, DxfUnits, read_dxf};
use crate::geometry::Transform;

/// How many undo steps are kept.
const UNDO_LIMIT: usize = 100;

/// What a DXF file contains, shown to the user before importing (e.g. to ask about units).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DxfInfo {
    pub units: DxfUnits,
    pub shape_count: usize,
}

/// Result of an import, for the pop-up message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportReport {
    pub objects_added: usize,
    /// Layers that received objects, in order.
    pub layers: Vec<String>,
    /// Entities not imported, with counts, e.g. {"TEXT": 2}.
    pub skipped: BTreeMap<String, usize>,
    /// Scale factor applied (25.4 when converting inches to mm).
    pub scale: f64,
}

/// Box selection rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoxMode {
    /// Select shapes entirely inside the box (dragged left to right).
    Window,
    /// Select shapes the box touches (dragged right to left).
    Crossing,
}

#[derive(Debug, Clone)]
pub struct Project {
    doc: Document,
    undo: Vec<Document>,
    redo: Vec<Document>,
    display: DisplayData,
}

impl Default for Project {
    fn default() -> Self {
        Self::new()
    }
}

impl Project {
    pub fn new() -> Self {
        let doc = Document::new("Untitled");
        let display = display_data(&doc);
        Self {
            doc,
            undo: Vec::new(),
            redo: Vec::new(),
            display,
        }
    }

    pub fn document(&self) -> &Document {
        &self.doc
    }

    pub fn display(&self) -> &DisplayData {
        &self.display
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Runs `change` on the document as one undoable step.
    fn edit(&mut self, change: impl FnOnce(&mut Document)) {
        let before = self.doc.clone();
        change(&mut self.doc);
        if self.doc != before {
            self.undo.push(before);
            if self.undo.len() > UNDO_LIMIT {
                self.undo.remove(0);
            }
            self.redo.clear();
            self.display = display_data(&self.doc);
        }
    }

    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(prev) => {
                self.redo.push(std::mem::replace(&mut self.doc, prev));
                self.display = display_data(&self.doc);
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.redo.pop() {
            Some(next) => {
                self.undo.push(std::mem::replace(&mut self.doc, next));
                self.display = display_data(&self.doc);
                true
            }
            None => false,
        }
    }

    /// Reads a DXF without importing it.
    pub fn dxf_info(text: &str) -> Result<DxfInfo, DxfError> {
        let drawing = read_dxf(text)?;
        Ok(DxfInfo {
            units: drawing.units,
            shape_count: drawing.shapes.len(),
        })
    }

    /// Imports a DXF into the current document as one undo step. `scale` multiplies all
    /// coordinates (1.0 = numbers are mm, 25.4 = numbers are inches).
    pub fn import_dxf(&mut self, text: &str, scale: f64) -> Result<ImportReport, DxfError> {
        let drawing = read_dxf(text)?;
        let t = Transform::scale(scale, scale);
        let mut report = ImportReport {
            objects_added: 0,
            layers: Vec::new(),
            skipped: drawing.skipped,
            scale,
        };
        self.edit(|doc| {
            let mut next_id = doc.max_id() + 1;
            for (layer, shape) in drawing.shapes {
                // A uniform scale never fails, but stay safe.
                let Some(shape) = shape.transformed(&t) else {
                    continue;
                };
                let li = doc.layer_index(&layer);
                doc.layers[li]
                    .objects
                    .push(DrawingObject { id: next_id, shape });
                next_id += 1;
                report.objects_added += 1;
                if !report.layers.contains(&layer) {
                    report.layers.push(layer);
                }
            }
        });
        Ok(report)
    }

    /// Deletes objects by id as one undo step. Returns how many were deleted.
    pub fn delete(&mut self, ids: &[u32]) -> usize {
        let ids: HashSet<u32> = ids.iter().copied().collect();
        let mut count = 0;
        self.edit(|doc| {
            for layer in &mut doc.layers {
                let before = layer.objects.len();
                layer.objects.retain(|o| !ids.contains(&o.id));
                count += before - layer.objects.len();
            }
        });
        count
    }

    /// The object nearest to (x, y) within `tolerance` mm, if any.
    pub fn pick(&self, x: f64, y: f64, tolerance: f64) -> Option<u32> {
        self.display
            .shapes
            .iter()
            .map(|s| (s.id, distance_to_shape(s, x, y)))
            .filter(|(_, d)| *d <= tolerance)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }

    /// Objects selected by a box from (x0, y0) to (x1, y1).
    pub fn pick_box(&self, x0: f64, y0: f64, x1: f64, y1: f64, mode: BoxMode) -> Vec<u32> {
        let r = Rect {
            min_x: x0.min(x1),
            min_y: y0.min(y1),
            max_x: x0.max(x1),
            max_y: y0.max(y1),
        };
        self.display
            .shapes
            .iter()
            .filter(|s| match mode {
                BoxMode::Window => !s.points.is_empty() && s.points.iter().all(|p| r.contains(*p)),
                BoxMode::Crossing => shape_touches_rect(s, &r),
            })
            .map(|s| s.id)
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------
// Hit-testing on the display approximation (within DISPLAY_TOLERANCE of the true shape).

struct Rect {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Rect {
    fn contains(&self, p: [f64; 2]) -> bool {
        p[0] >= self.min_x && p[0] <= self.max_x && p[1] >= self.min_y && p[1] <= self.max_y
    }
}

fn segments(s: &DisplayShape) -> impl Iterator<Item = ([f64; 2], [f64; 2])> + '_ {
    let n = s.points.len();
    let count = if s.markers || n < 2 {
        0
    } else if s.closed {
        n
    } else {
        n - 1
    };
    (0..count).map(move |i| (s.points[i], s.points[(i + 1) % n]))
}

fn distance_to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
    };
    (p[0] - (a[0] + t * dx)).hypot(p[1] - (a[1] + t * dy))
}

fn distance_to_shape(s: &DisplayShape, x: f64, y: f64) -> f64 {
    let p = [x, y];
    let to_points = s
        .points
        .iter()
        .map(|q| (q[0] - x).hypot(q[1] - y))
        .fold(f64::INFINITY, f64::min);
    if s.markers || s.points.len() < 2 {
        return to_points;
    }
    segments(s)
        .map(|(a, b)| distance_to_segment(p, a, b))
        .fold(to_points, f64::min)
}

fn shape_touches_rect(s: &DisplayShape, r: &Rect) -> bool {
    s.points.iter().any(|p| r.contains(*p)) || segments(s).any(|(a, b)| segment_hits_rect(a, b, r))
}

/// Liang–Barsky clipping: does segment a→b pass through the rectangle?
fn segment_hits_rect(a: [f64; 2], b: [f64; 2], r: &Rect) -> bool {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let mut t0: f64 = 0.0;
    let mut t1: f64 = 1.0;
    for (p, q) in [
        (-dx, a[0] - r.min_x),
        (dx, r.max_x - a[0]),
        (-dy, a[1] - r.min_y),
        (dy, r.max_y - a[1]),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}
