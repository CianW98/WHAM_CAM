//! The open project: the document plus undo/redo history and selection hit-testing.
//! The app layer holds one `Project` and calls these methods; the interface never edits
//! the document directly.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::dimension;
use crate::display::{
    CutWidthStep, Playback, PlaybackTool, ToolpathDisplay, cut_width_steps, playback,
    toolpath_display,
};
use crate::display::{DisplayData, DisplayShape, display_data, display_shape};
use crate::document::{Document, DrawingObject, is_hex_color};
use crate::draw::{self, Draft, DrawTool, Pick, PickKind, Snap, SnapKind, SnapOptions};
use crate::drill;
use crate::dxf::{DxfError, DxfOptions, DxfUnits, read_dxf};
use crate::edit;
use crate::gcode_read::check_program;
use crate::geometry::{Point, Shape, Transform};
use crate::operation::{
    DrillSettings, Operation, OperationKind, PocketSettings, ProfileSettings, ProfileSide,
    ToolSource,
};
use crate::path;
use crate::pocket;
use crate::post::{Header, JobOperation, PostFormat, Program, write_program};
use crate::profile;
use crate::project_file;
use crate::safety;
use crate::tooling::{PickedTool, Tool};
use crate::toolpath::Toolpath;
use crate::transform::{TransformOp, bounds_of, centre, transform_shapes};

/// How many undo steps are kept.
const UNDO_LIMIT: usize = 100;

/// What a DXF file contains, shown to the user before importing (e.g. to ask about units).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DxfInfo {
    pub units: DxfUnits,
    pub shape_count: usize,
    /// Splines and ellipses, which are converted into polylines on import.
    pub curve_count: usize,
}

/// What a Dimension tool click is (see [`Project::dimension_click`]).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DimensionClick {
    /// Where the click counts: the snapped point, or the click itself for a pick.
    pub point: Point,
    /// What a point snapped to (`None` for a pick).
    pub kind: SnapKind,
    /// The circle, arc or line it picked, if any.
    pub pick: Option<Pick>,
    /// This click completes the dimension.
    pub finishes: bool,
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
    /// Splines and ellipses converted into polylines of arcs and lines.
    pub curves_converted: usize,
}

/// Result of a join, for the pop-up message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JoinReport {
    /// The new polylines' ids.
    pub ids: Vec<u32>,
    /// How many shapes went into them.
    pub joined: usize,
    /// How many of the new polylines are closed.
    pub closed: usize,
    /// Selected shapes left as they were (nothing to join to, or not a line, arc or open polyline).
    pub skipped: usize,
}

/// Result of breaking at intersections, for the pop-up message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BreakReport {
    /// The pieces' ids.
    pub ids: Vec<u32>,
    /// How many shapes were broken.
    pub broken: usize,
    /// How many distinct crossing points were found.
    pub crossings: usize,
}

/// The state of an operation's toolpath, for the tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationState {
    pub id: u32,
    /// "none" (never generated), "ok", "outdated" (settings or shapes changed since) or "error".
    pub status: String,
    /// The error, for "error" (and an out-of-date error).
    pub message: Option<String>,
    /// True if it machines any open shapes (lines, arcs, open polylines): a Profile then only
    /// offers On line.
    pub has_open_shapes: bool,
    /// Where its tool comes from: "operation" (its own: library or custom), "part", "machining",
    /// or "none" (follows the tree but no tool is set there).
    pub tool_from: String,
    /// The tool it uses (None for "none").
    pub tool_number: Option<u32>,
    pub tool_diameter: Option<f64>,
    /// The tool's name (None for "none").
    pub tool_name: Option<String>,
}

/// How an operation gets its tool (for [`Project::set_operation_tool`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToolSetting {
    /// Use the part's tool, or the Machining tool.
    Inherit,
    /// Its own tool from a tool library.
    Library(PickedTool),
    /// Its own tool with values set in the operation, starting from the tool it was using.
    Custom,
}

/// The message when an operation follows the tree but no tool is set there.
pub const NO_TOOL: &str =
    "No tool selected. Choose a tool for Machining, the part, or the operation.";

/// Result of Generate toolpaths, for the pop-up message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerateReport {
    pub generated: usize,
    /// (operation name, error) for each operation that failed.
    pub errors: Vec<(String, String)>,
    /// Things to know that didn't stop generating (e.g. the library isn't on this computer).
    pub warnings: Vec<String>,
}

/// A generated toolpath (or its error), with what it was made from.
#[derive(Debug, Clone)]
struct Generated {
    operation: Operation,
    /// The tool it was generated with (its own, its part's or Machining's), if it had one.
    tool: Option<Tool>,
    shapes: Vec<Option<Shape>>,
    result: Result<Toolpath, String>,
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
    /// Copied shapes with the layer each came from. Not part of the document or of undo.
    clipboard: Vec<(String, Shape)>,
    /// The tools of the project's tool library on this computer, by number (set by the app):
    /// what is in the machine's tool changer. None = the project names a library that isn't on
    /// this computer, so it can't be checked against. Not part of the document.
    library_tools: Option<Vec<PickedTool>>,
    /// The document as last saved or opened, to tell whether there are unsaved changes.
    saved: Document,
    /// Generated toolpaths by operation id. Not part of the document: regenerated on request.
    toolpaths: BTreeMap<u32, Generated>,
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
            saved: doc.clone(),
            doc,
            undo: Vec::new(),
            redo: Vec::new(),
            display,
            clipboard: Vec::new(),
            library_tools: Some(Vec::new()),
            toolpaths: BTreeMap::new(),
        }
    }

    // --- Project files ---------------------------------------------------------------------

    /// Starts a new, empty project. The clipboard is kept.
    pub fn new_project(&mut self) {
        self.replace_document(Document::new("Untitled"));
    }

    /// Opens a project from project-file text, replacing the current one with a fresh undo
    /// history. It is named `name` (the file name without its extension). On an error nothing
    /// changes. The clipboard is kept.
    pub fn open(&mut self, text: &str, name: &str) -> Result<(), String> {
        let mut doc = project_file::from_text(text)?;
        doc.name = name.to_string();
        self.replace_document(doc);
        Ok(())
    }

    fn replace_document(&mut self, doc: Document) {
        self.toolpaths.clear();
        self.saved = doc.clone();
        self.display = display_data(&doc);
        self.doc = doc;
        self.undo.clear();
        self.redo.clear();
    }

    /// The project as project-file text, named `name` (the file name without its extension).
    /// The name change is not an undo step. Call [`Project::mark_saved`] once the file is written.
    pub fn save_text(&mut self, name: &str) -> String {
        self.set_name(name);
        project_file::to_text(&self.doc)
    }

    /// Records that the project as it is now has been saved.
    pub fn mark_saved(&mut self) {
        self.saved = self.doc.clone();
    }

    /// True if the project has changed since it was last saved or opened (undoing back to that
    /// state counts as unchanged).
    pub fn is_modified(&self) -> bool {
        self.doc != self.saved
    }

    /// Sets the project name everywhere in the history, so undo doesn't bring back an old name.
    fn set_name(&mut self, name: &str) {
        for doc in std::iter::once(&mut self.doc)
            .chain(&mut self.undo)
            .chain(&mut self.redo)
        {
            doc.name = name.to_string();
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
        let drawing = read_dxf(text, &DxfOptions::default())?;
        Ok(DxfInfo {
            units: drawing.units,
            shape_count: drawing.shapes.len(),
            curve_count: drawing.curves,
        })
    }

    /// Imports a DXF into the current document as one undo step. `scale` multiplies all
    /// coordinates (1.0 = numbers are mm, 25.4 = numbers are inches). Splines and ellipses become
    /// polylines of arcs and lines within `curve_tolerance` mm of the true curve.
    pub fn import_dxf(
        &mut self,
        text: &str,
        scale: f64,
        curve_tolerance: f64,
    ) -> Result<ImportReport, DxfError> {
        let drawing = read_dxf(
            text,
            &DxfOptions {
                scale,
                curve_tolerance,
            },
        )?;
        let mut report = ImportReport {
            objects_added: 0,
            layers: Vec::new(),
            skipped: drawing.skipped,
            scale,
            curves_converted: drawing.curves,
        };
        self.edit(|doc| {
            let first_id = doc.max_id() + 1;
            for (id, (layer, shape)) in (first_id..).zip(drawing.shapes) {
                let li = doc.layer_index(&layer);
                doc.layers[li].objects.push(DrawingObject { id, shape });
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

    /// Deletes a layer and everything on it, as one undo step. If no layers would be left,
    /// an empty "Default" layer takes its place so there is always somewhere to draw.
    /// Returns false if there is no layer with that name.
    pub fn delete_layer(&mut self, name: &str) -> bool {
        let Some(index) = self.doc.layers.iter().position(|l| l.name == name) else {
            return false;
        };
        self.edit(|doc| {
            doc.layers.remove(index);
            if doc.layers.is_empty() {
                doc.layer_index("Default");
            }
        });
        true
    }

    /// Adds a new, empty layer called "Layer 1" (or the next unused number) with the next free
    /// colour, as one undo step. Returns its name.
    pub fn add_layer(&mut self) -> String {
        let name = (1..)
            .map(|n| format!("Layer {n}"))
            .find(|n| !self.doc.layers.iter().any(|l| &l.name == n))
            .expect("some number is free");
        self.edit(|doc| {
            doc.layer_index(&name);
        });
        name
    }

    /// Renames a layer as one undo step. The new name is trimmed, must not be empty and
    /// must not already be used by another layer. Errors are plain-English messages.
    pub fn rename_layer(&mut self, old: &str, new: &str) -> Result<(), String> {
        let new = new.trim();
        let index = self.layer_position(old)?;
        if new.is_empty() {
            return Err("A layer name can't be empty.".to_string());
        }
        if new == old {
            return Ok(());
        }
        if self.doc.layers.iter().any(|l| l.name == new) {
            return Err(format!("There is already a layer called \"{new}\"."));
        }
        self.edit(|doc| doc.layers[index].name = new.to_string());
        Ok(())
    }

    /// Sets a layer's display colour ("#rrggbb") as one undo step.
    pub fn set_layer_color(&mut self, name: &str, color: &str) -> Result<(), String> {
        let index = self.layer_position(name)?;
        if !is_hex_color(color) {
            return Err(format!("\"{color}\" is not a colour (expected #rrggbb)."));
        }
        let color = color.to_ascii_lowercase();
        self.edit(|doc| doc.layers[index].color = color);
        Ok(())
    }

    fn layer_position(&self, name: &str) -> Result<usize, String> {
        self.doc
            .layers
            .iter()
            .position(|l| l.name == name)
            .ok_or_else(|| format!("There is no layer called \"{name}\"."))
    }

    /// Adds a shape to a layer (created if it doesn't exist) as one undo step.
    /// Returns the new object's id. Errors are plain-English messages.
    pub fn add_shape(&mut self, layer: &str, shape: Shape) -> Result<u32, String> {
        draw::validate(&shape)?;
        let id = self.doc.max_id() + 1;
        self.edit(|doc| {
            let li = doc.layer_index(layer);
            doc.layers[li].objects.push(DrawingObject { id, shape });
        });
        Ok(id)
    }

    /// Finishes a drawn shape and adds it to a layer, as one undo step. Returns the new id.
    pub fn add_draft(&mut self, layer: &str, draft: &Draft) -> Result<u32, String> {
        let shape = if draft.tool == DrawTool::Dimension {
            let kinds = Self::pick_kinds(draft);
            if !dimension::complete(&kinds) {
                return Err("This dimension needs more clicks.".into());
            }
            Shape::Dimension(self.smart_dimension(draft, None).ok_or_else(|| {
                if kinds.len() == 3 && kinds[0].is_some() {
                    "Those lines lie on top of each other: there is nothing to measure."
                } else {
                    "That can't be dimensioned."
                }
                .to_string()
            })?)
        } else {
            draw::finish(draft)?
        };
        draw::validate(&shape)?;
        self.add_shape(layer, shape)
    }

    /// What each click of a dimension draft picked (`None` for a point).
    fn pick_kinds(draft: &Draft) -> Vec<Option<PickKind>> {
        (0..draft.points.len())
            .map(|i| draft.picks.get(i).copied().flatten().map(|p| p.kind))
            .collect()
    }

    /// What a Dimension tool click at (x, y) is, given the clicks so far (decided 2026-11-20):
    /// - First click: an end or centre snap is a point; otherwise touching a circle, arc or line
    ///   picks it (even at its midpoint); otherwise a grid/free point.
    /// - After a line: touching another line picks it (angle, or distance if parallel);
    ///   anything else is where the line's length goes.
    /// - Otherwise: a snapped point (the second point, or where the dimension goes).
    ///
    /// `finishes` says whether this click completes the dimension.
    pub fn dimension_click(
        &self,
        draft: &Draft,
        x: f64,
        y: f64,
        tolerance: f64,
        options: SnapOptions,
    ) -> DimensionClick {
        let at = Point::new(x, y);
        let kinds = Self::pick_kinds(draft);
        let snapped = || draw::snap(&self.doc, at, tolerance, options, &[]);
        let picked = |pick: Pick| (at, SnapKind::None, Some(pick));
        let (point, kind, pick) = match kinds[..] {
            [] => {
                let point_snap = options
                    .objects
                    .then(|| {
                        draw::object_snap(&self.doc, at, tolerance, &[], |k| {
                            matches!(k, SnapKind::End | SnapKind::Centre)
                        })
                    })
                    .flatten();
                match (point_snap, self.nearest_pick(at, tolerance, None)) {
                    (Some(s), _) => (s.point, s.kind, None),
                    (None, Some(pick)) => picked(pick),
                    (None, None) => {
                        let s = draw::grid_snap(&self.doc, at, options);
                        (s.point, s.kind, None)
                    }
                }
            }
            [Some(PickKind::Line)] => {
                let first = draft.picks[0].map(|p| (p.id, draft.points[0]));
                match self.nearest_pick(at, tolerance, first) {
                    Some(pick) if pick.kind == PickKind::Line => picked(pick),
                    _ => {
                        let s = snapped();
                        (s.point, s.kind, None)
                    }
                }
            }
            _ => {
                let s = snapped();
                (s.point, s.kind, None)
            }
        };
        let mut next = kinds;
        next.push(pick.map(|p| p.kind));
        DimensionClick {
            point,
            kind,
            pick,
            finishes: dimension::complete(&next),
        }
    }

    /// The circle, arc or line nearest to `at` within `tolerance` mm. With `not_line` (the id and
    /// click point of a line already picked), that same line segment is left out.
    fn nearest_pick(
        &self,
        at: Point,
        tolerance: f64,
        not_line: Option<(u32, Point)>,
    ) -> Option<Pick> {
        let same = |id: u32, l: dimension::PickedLine| {
            not_line.is_some_and(|(nid, click)| {
                nid == id
                    && self
                        .object_shape(id)
                        .and_then(|s| dimension::pick_line(s, click))
                        .is_some_and(|first| first.a == l.a && first.b == l.b)
            })
        };
        let mut best: Option<(f64, Pick)> = None;
        for o in self.doc.objects() {
            let mut consider = |d: f64, kind: PickKind| {
                if d <= tolerance && best.is_none_or(|b| d < b.0) {
                    best = Some((d, Pick { id: o.id, kind }));
                }
            };
            if let Some(c) = dimension::pick_circle(&o.shape, at) {
                consider(c.distance, PickKind::Circle);
            }
            if let Some(l) = dimension::pick_line(&o.shape, at)
                && !same(o.id, l)
            {
                consider(l.distance, PickKind::Line);
            }
        }
        best.map(|b| b.1)
    }

    fn object_shape(&self, id: u32) -> Option<&Shape> {
        self.doc.objects().find(|o| o.id == id).map(|o| &o.shape)
    }

    /// The dimension a Dimension draft makes, with `cursor` (if any) as the next click: the
    /// clicks are read as in [`Project::dimension_click`].
    fn smart_dimension(
        &self,
        draft: &Draft,
        cursor: Option<Point>,
    ) -> Option<dimension::Dimension> {
        let mut clicks: Vec<(Point, Option<Pick>)> = draft
            .points
            .iter()
            .enumerate()
            .map(|(i, p)| (*p, draft.picks.get(i).copied().flatten()))
            .collect();
        clicks.extend(cursor.map(|c| (c, None)));
        let circle = |i: usize| {
            let (click, pick) = clicks[i];
            dimension::pick_circle(self.object_shape(pick?.id)?, click)
        };
        let line = |i: usize| {
            let (click, pick) = clicks[i];
            dimension::pick_line(self.object_shape(pick?.id)?, click)
        };
        let kinds: Vec<Option<PickKind>> = clicks.iter().map(|c| c.1.map(|p| p.kind)).collect();
        use PickKind::{Circle, Line};
        match kinds[..] {
            [Some(Circle), None] => dimension::circular(circle(0)?, clicks[1].0),
            [Some(Line), None] => {
                let l = line(0)?;
                dimension::aligned(l.a, l.b, clicks[1].0)
            }
            [Some(Line), Some(Line), None] => {
                let (l1, l2, at) = (line(0)?, line(1)?, clicks[2].0);
                dimension::angle(l1, l2, at)
                    .or_else(|| dimension::parallel_distance(l1, clicks[0].0, l2, at))
            }
            // Second point following the cursor: the line runs straight between the points.
            [None, None] => dimension::aligned(clicks[0].0, clicks[1].0, clicks[1].0),
            [None, None, None] => dimension::aligned(clicks[0].0, clicks[1].0, clicks[2].0),
            _ => None,
        }
    }

    /// Replaces an object's shape (e.g. after editing values in Properties) as one undo step.
    pub fn set_shape(&mut self, id: u32, shape: Shape) -> Result<(), String> {
        draw::validate(&shape)?;
        let (li, oi) = self
            .doc
            .layers
            .iter()
            .enumerate()
            .find_map(|(li, l)| l.objects.iter().position(|o| o.id == id).map(|oi| (li, oi)))
            .ok_or_else(|| format!("There is no object with id {id}."))?;
        self.edit(|doc| doc.layers[li].objects[oi].shape = shape);
        Ok(())
    }

    /// Sets the grid spacing in mm as one undo step.
    pub fn set_grid_spacing(&mut self, mm: f64) -> Result<(), String> {
        if !(mm.is_finite() && mm > 0.0) {
            return Err("The grid spacing must be more than 0 mm.".to_string());
        }
        self.edit(|doc| doc.grid_spacing = mm);
        Ok(())
    }

    /// Snaps a cursor position while drawing. `extra` are the points of the shape being drawn.
    pub fn snap(
        &self,
        x: f64,
        y: f64,
        tolerance: f64,
        options: SnapOptions,
        extra: &[Point],
    ) -> Snap {
        draw::snap(&self.doc, Point::new(x, y), tolerance, options, extra)
    }

    /// The rubber-band preview of a shape being drawn, with the cursor as its next point.
    pub fn preview(&self, draft: &Draft, cursor: Option<Point>) -> Option<DisplayShape> {
        let shape = match draft.tool {
            DrawTool::Dimension => self.smart_dimension(draft, cursor).map(Shape::Dimension),
            _ => draw::preview(draft, cursor),
        };
        shape.map(|shape| display_shape(0, &shape))
    }

    /// The selected objects' (layer index, object index, shape), in document order.
    fn located(&self, ids: &[u32]) -> Vec<(usize, usize, Shape)> {
        let ids: HashSet<u32> = ids.iter().copied().collect();
        let mut out = Vec::new();
        for (li, layer) in self.doc.layers.iter().enumerate() {
            for (oi, obj) in layer.objects.iter().enumerate() {
                if ids.contains(&obj.id) {
                    out.push((li, oi, obj.shape.clone()));
                }
            }
        }
        out
    }

    /// Moves, rotates, scales or mirrors objects as one undo step. Rotate, scale and mirror
    /// work around the centre of the objects' bounding box. Nothing changes on an error.
    pub fn transform(&mut self, ids: &[u32], op: TransformOp) -> Result<(), String> {
        let found = self.located(ids);
        let shapes: Vec<Shape> = found.iter().map(|f| f.2.clone()).collect();
        let moved = transform_shapes(&shapes, op)?;
        self.edit(|doc| {
            for ((li, oi, _), shape) in found.iter().zip(moved) {
                doc.layers[*li].objects[*oi].shape = shape;
            }
        });
        Ok(())
    }

    /// Centre of the objects' bounding box (what rotate, scale and mirror work around), or
    /// `None` if none of the ids exist.
    pub fn selection_centre(&self, ids: &[u32]) -> Option<Point> {
        let found = self.located(ids);
        bounds_of(found.iter().map(|f| &f.2)).map(centre)
    }

    /// What `transform` would give, for drawing a preview. Empty on an error.
    pub fn transform_preview(&self, ids: &[u32], op: TransformOp) -> Vec<DisplayShape> {
        let shapes: Vec<Shape> = self.located(ids).into_iter().map(|f| f.2).collect();
        transform_shapes(&shapes, op)
            .unwrap_or_default()
            .iter()
            .map(|s| display_shape(0, s))
            .collect()
    }

    /// Copies objects to the clipboard (replacing what was there). Returns how many.
    pub fn copy(&mut self, ids: &[u32]) -> usize {
        let found = self.located(ids);
        if !found.is_empty() {
            self.clipboard = found
                .into_iter()
                .map(|(li, _, shape)| (self.doc.layers[li].name.clone(), shape))
                .collect();
        }
        self.clipboard.len()
    }

    /// Copies objects to the clipboard, then deletes them (one undo step). Returns how many.
    pub fn cut(&mut self, ids: &[u32]) -> usize {
        if self.located(ids).is_empty() {
            return 0;
        }
        self.copy(ids);
        self.delete(ids)
    }

    pub fn has_clipboard(&self) -> bool {
        !self.clipboard.is_empty()
    }

    /// The clipboard shapes moved so their common centre is at `at`.
    fn clipboard_at(&self, at: Point) -> Vec<(String, Shape)> {
        let Some(b) = bounds_of(self.clipboard.iter().map(|c| &c.1)) else {
            return Vec::new();
        };
        let c = centre(b);
        let t = Transform::translate(at.x - c.x, at.y - c.y);
        self.clipboard
            .iter()
            .filter_map(|(layer, s)| s.transformed(&t).map(|s| (layer.clone(), s)))
            .collect()
    }

    /// Preview of pasting with the clipboard's centre at `at`.
    pub fn paste_preview(&self, at: Point) -> Vec<DisplayShape> {
        self.clipboard_at(at)
            .iter()
            .map(|(_, s)| display_shape(0, s))
            .collect()
    }

    /// Pastes the clipboard with its centre at `at`, onto the layers the shapes were copied
    /// from (created if needed), as one undo step. Returns the new objects' ids.
    pub fn paste(&mut self, at: Point) -> Vec<u32> {
        let shapes = self.clipboard_at(at);
        let mut ids = Vec::new();
        self.edit(|doc| {
            let first = doc.max_id() + 1;
            for (id, (layer, shape)) in (first..).zip(shapes) {
                let li = doc.layer_index(&layer);
                doc.layers[li].objects.push(DrawingObject { id, shape });
                ids.push(id);
            }
        });
        ids
    }

    /// Replaces objects with new shapes (in their place, on their layer) as one undo step.
    /// An empty list removes the object. Returns the new objects' ids in order.
    fn replace_objects(&mut self, with: HashMap<u32, Vec<Shape>>) -> Vec<u32> {
        let mut ids = Vec::new();
        self.edit(|doc| {
            let mut next = doc.max_id() + 1;
            for layer in &mut doc.layers {
                let old = std::mem::take(&mut layer.objects);
                for obj in old {
                    match with.get(&obj.id) {
                        None => layer.objects.push(obj),
                        Some(shapes) => {
                            for shape in shapes {
                                layer.objects.push(DrawingObject {
                                    id: next,
                                    shape: shape.clone(),
                                });
                                ids.push(next);
                                next += 1;
                            }
                        }
                    }
                }
            }
        });
        ids
    }

    /// Joins lines, arcs and open polylines whose ends are within `tolerance` mm into
    /// polylines, as one undo step. Each new polyline takes the place (and layer) of its first
    /// piece.
    pub fn join(&mut self, ids: &[u32], tolerance: f64) -> Result<JoinReport, String> {
        if !(tolerance.is_finite() && tolerance > 0.0) {
            return Err("The tolerance must be more than 0.".into());
        }
        let found = self.located(ids);
        let shapes: Vec<Shape> = found.iter().map(|f| f.2.clone()).collect();
        let joined = edit::join(&shapes, tolerance);
        let id_of = |i: usize| self.doc.layers[found[i].0].objects[found[i].1].id;
        let mut with = HashMap::new();
        for j in &joined {
            with.insert(
                id_of(j.sources[0]),
                vec![Shape::Polyline(j.polyline.clone())],
            );
            for &i in &j.sources[1..] {
                with.insert(id_of(i), Vec::new());
            }
        }
        let used: usize = joined.iter().map(|j| j.sources.len()).sum();
        let report = JoinReport {
            ids: Vec::new(),
            joined: used,
            closed: joined.iter().filter(|j| j.polyline.closed).count(),
            skipped: found.len() - used,
        };
        let ids = if with.is_empty() {
            Vec::new()
        } else {
            self.replace_objects(with)
        };
        Ok(JoinReport { ids, ..report })
    }

    /// Breaks polylines and rectangles into lines and arcs, as one undo step. Returns the
    /// pieces' ids (empty if nothing could be exploded).
    pub fn explode(&mut self, ids: &[u32]) -> Vec<u32> {
        let with: HashMap<u32, Vec<Shape>> = self
            .located(ids)
            .into_iter()
            .filter_map(|(li, oi, shape)| {
                let pieces = edit::explode(&shape)?;
                Some((self.doc.layers[li].objects[oi].id, pieces))
            })
            .collect();
        if with.is_empty() {
            return Vec::new();
        }
        self.replace_objects(with)
    }

    /// Closes open polylines, as one undo step. Returns how many were closed.
    pub fn close_polylines(&mut self, ids: &[u32]) -> usize {
        let closed: Vec<(usize, usize, Shape)> = self
            .located(ids)
            .into_iter()
            .filter_map(|(li, oi, shape)| match shape {
                Shape::Polyline(pl) => Some((li, oi, Shape::Polyline(edit::close_polyline(&pl)?))),
                _ => None,
            })
            .collect();
        let count = closed.len();
        self.edit(|doc| {
            for (li, oi, shape) in closed {
                doc.layers[li].objects[oi].shape = shape;
            }
        });
        count
    }

    /// Breaks the shapes where they cross or touch each other, as one undo step. The pieces
    /// take the place (and layer) of the shape they came from.
    pub fn break_at_intersections(&mut self, ids: &[u32]) -> BreakReport {
        let found = self.located(ids);
        let shapes: Vec<Shape> = found.iter().map(|f| f.2.clone()).collect();
        let (broken, crossings) = edit::break_at_intersections(&shapes);
        let with: HashMap<u32, Vec<Shape>> = broken
            .into_iter()
            .map(|b| {
                let (li, oi, _) = found[b.source];
                (self.doc.layers[li].objects[oi].id, b.pieces)
            })
            .collect();
        let broken = with.len();
        let ids = if with.is_empty() {
            Vec::new()
        } else {
            self.replace_objects(with)
        };
        BreakReport {
            ids,
            broken,
            crossings,
        }
    }

    // --- Operations ---------------------------------------------------------------------------

    /// Where an operation is: (part index, operation index).
    fn find_operation(&self, id: u32) -> Option<(usize, usize)> {
        self.doc.parts.iter().enumerate().find_map(|(pi, part)| {
            part.operations
                .iter()
                .position(|o| o.id == id)
                .map(|oi| (pi, oi))
        })
    }

    pub fn operation(&self, id: u32) -> Option<&Operation> {
        self.find_operation(id)
            .map(|(pi, oi)| &self.doc.parts[pi].operations[oi])
    }

    /// The selected shapes as closed outlines, or why they can't be pocketed.
    fn outlines(&self, ids: &[u32]) -> Result<Vec<(u32, crate::geometry::Polyline)>, String> {
        ids.iter()
            .map(|&id| {
                let shape = self
                    .doc
                    .objects()
                    .find(|o| o.id == id)
                    .map(|o| &o.shape)
                    .ok_or_else(|| format!("Shape {id} no longer exists."))?;
                Ok((id, pocket::closed_outline(id, shape)?))
            })
            .collect()
    }

    /// The shapes as paths to profile (closed or open), with their ids.
    fn profile_paths(&self, ids: &[u32]) -> Result<Vec<(u32, crate::geometry::Polyline)>, String> {
        ids.iter()
            .map(|&id| {
                let shape = self
                    .doc
                    .objects()
                    .find(|o| o.id == id)
                    .map(|o| &o.shape)
                    .ok_or_else(|| format!("Shape {id} no longer exists."))?;
                Ok((id, path::profile_path(id, shape)?))
            })
            .collect()
    }

    /// The hole positions of the shapes, with their ids.
    fn drill_holes(&self, ids: &[u32]) -> Result<Vec<(u32, Vec<drill::Hole>)>, String> {
        ids.iter()
            .map(|&id| {
                let shape = self
                    .doc
                    .objects()
                    .find(|o| o.id == id)
                    .map(|o| &o.shape)
                    .ok_or_else(|| format!("Shape {id} no longer exists."))?;
                Ok((id, drill::drill_points(id, shape)?))
            })
            .collect()
    }

    /// Adds a Drill of the given shapes (point lists, circles, arcs) to the first part, as one
    /// undo step. Returns its id.
    pub fn add_drill(&mut self, shape_ids: &[u32]) -> Result<u32, String> {
        if shape_ids.is_empty() {
            return Err("Select the point lists, circles or arcs to drill first.".into());
        }
        self.add_operation(
            shape_ids,
            "Drill",
            OperationKind::Drill(DrillSettings::default()),
        )
    }

    /// Checks the shapes suit an operation of this kind.
    fn check_shapes(&self, kind: &OperationKind, ids: &[u32]) -> Result<(), String> {
        match kind {
            OperationKind::Profile(_) => self.profile_paths(ids).map(|_| ()),
            OperationKind::Pocket(_) => self.outlines(ids).map(|_| ()),
            OperationKind::Drill(_) => self.drill_holes(ids).map(|_| ()),
        }
    }

    /// Adds a Pocket of the given shapes to the first part, as one undo step. Returns its id.
    /// Refuses shapes that can't be pocketed (open shapes, point lists, ...).
    pub fn add_pocket(&mut self, shape_ids: &[u32]) -> Result<u32, String> {
        if shape_ids.is_empty() {
            return Err("Select the closed shapes to pocket first.".into());
        }
        self.add_operation(
            shape_ids,
            "Pocket",
            OperationKind::Pocket(PocketSettings::default()),
        )
    }

    /// Adds a Profile of the given shapes to the first part, as one undo step. Returns its id.
    /// It cuts Outside, or On line if any of the shapes are open. Refuses shapes that can't be
    /// profiled (point lists).
    pub fn add_profile(&mut self, shape_ids: &[u32]) -> Result<u32, String> {
        if shape_ids.is_empty() {
            return Err("Select the shapes to profile first.".into());
        }
        let paths = self.profile_paths(shape_ids)?;
        let mut settings = ProfileSettings::default();
        if paths.iter().any(|(_, pl)| !pl.closed) {
            settings.side = ProfileSide::OnLine;
        }
        self.add_operation(shape_ids, "Profile", OperationKind::Profile(settings))
    }

    fn add_operation(
        &mut self,
        shape_ids: &[u32],
        base_name: &str,
        kind: OperationKind,
    ) -> Result<u32, String> {
        self.check_shapes(&kind, shape_ids)?;
        let id = self
            .doc
            .parts
            .iter()
            .flat_map(|p| &p.operations)
            .map(|o| o.id)
            .max()
            .unwrap_or(0)
            + 1;
        let name = (1..)
            .map(|n| format!("{base_name}{n}"))
            .find(|n| {
                !self
                    .doc
                    .parts
                    .iter()
                    .flat_map(|p| &p.operations)
                    .any(|o| &o.name == n)
            })
            .expect("some number is free");
        let op = Operation {
            id,
            name,
            enabled: true,
            show_toolpath: true,
            shape_ids: shape_ids.to_vec(),
            common: Default::default(),
            kind,
        };
        self.edit(|doc| {
            if doc.parts.is_empty() {
                doc.parts.push(crate::document::Part {
                    name: "Part1".into(),
                    tool: None,
                    operations: Vec::new(),
                });
            }
            doc.parts[0].operations.push(op);
        });
        Ok(id)
    }

    /// Replaces an operation's settings (matched by id) as one undo step. Only checks that the
    /// name isn't empty and values are numbers; the full check happens when generating.
    pub fn set_operation(&mut self, op: Operation) -> Result<(), String> {
        let (pi, oi) = self
            .find_operation(op.id)
            .ok_or_else(|| format!("There is no operation with id {}.", op.id))?;
        if op.name.trim().is_empty() {
            return Err("An operation needs a name.".into());
        }
        if let Err(e) = op.validate()
            && e.starts_with("Every setting")
        {
            return Err(e);
        }
        let mut op = op;
        if op.common.tool_source != ToolSource::Inherit {
            let tool = op
                .common
                .tool
                .take()
                .ok_or("The operation's tool is missing.")?;
            // A custom tool's name follows its values (unless typed), and its values must
            // make sense, like a tool in the master list.
            let tool = if op.common.tool_source == ToolSource::Custom {
                let tool = tool.named();
                tool.validate()?;
                tool
            } else {
                tool
            };
            op.common.tool_diameter = tool.diameter;
            op.common.tool = Some(tool);
            // Checked when the custom tool or its number changes (Generate checks every time).
            let before = &self.doc.parts[pi].operations[oi].common;
            let changed = before.tool_source != op.common.tool_source
                || before.tool_number != op.common.tool_number
                || before.tool != op.common.tool;
            if op.common.tool_source == ToolSource::Custom && changed {
                self.check_custom_number(&op)?;
            }
        }
        self.edit(|doc| doc.parts[pi].operations[oi] = op);
        Ok(())
    }

    /// Sets how an operation gets its tool, as one undo step. Switching to Custom starts from
    /// the tool it was using (or a new 6 mm end mill if it had none), so nothing changes until
    /// a value is changed.
    pub fn set_operation_tool(&mut self, id: u32, setting: ToolSetting) -> Result<(), String> {
        let (pi, oi) = self
            .find_operation(id)
            .ok_or_else(|| format!("There is no operation with id {id}."))?;
        if let ToolSetting::Library(p) = &setting {
            p.check_usable()?;
        }
        let (_, current) = self.tool_source(pi, &self.doc.parts[pi].operations[oi]);
        let free = self.free_tool_number(id);
        self.edit(|doc| {
            let c = &mut doc.parts[pi].operations[oi].common;
            match setting {
                ToolSetting::Inherit => c.tool_source = ToolSource::Inherit,
                ToolSetting::Library(p) => {
                    c.tool_source = ToolSource::Library;
                    c.tool_number = p.number;
                    c.tool_diameter = p.tool.diameter;
                    c.tool = Some(p.tool);
                }
                ToolSetting::Custom => {
                    let p = current.unwrap_or(PickedTool {
                        number: 1,
                        tool: Tool::new_default(),
                    });
                    c.tool_source = ToolSource::Custom;
                    // A different tool from the one it copies, so it gets its own number.
                    c.tool_number = free;
                    c.tool_diameter = p.tool.diameter;
                    c.tool = Some(p.tool);
                }
            }
        });
        Ok(())
    }

    /// Tells the project the tools of its tool library on this computer (the app calls this
    /// before edits and generating): empty if the project has no library chosen, None if it
    /// names one that isn't on this computer. Tool numbers are checked against them.
    pub fn set_library_tools(&mut self, tools: Option<Vec<PickedTool>>) {
        self.library_tools = tools;
    }

    /// Tool numbers of the library on this computer (none if it isn't here).
    fn library_numbers(&self) -> Vec<u32> {
        self.library_tools
            .iter()
            .flatten()
            .map(|p| p.number)
            .collect()
    }

    /// Every tool the project names with a number, except operation `except`'s own tool:
    /// Machining's, each part's, and each operation's own (library or custom) tool.
    fn project_tools(&self, except: u32) -> Vec<PickedTool> {
        let mut tools: Vec<PickedTool> = self.doc.machining_tool.iter().cloned().collect();
        for part in &self.doc.parts {
            tools.extend(part.tool.iter().cloned());
            for op in &part.operations {
                let c = &op.common;
                if op.id != except
                    && c.tool_source != ToolSource::Inherit
                    && let Some(tool) = &c.tool
                {
                    tools.push(PickedTool {
                        number: c.tool_number,
                        tool: tool.clone(),
                    });
                }
            }
        }
        tools
    }

    /// The lowest tool number not in the active library and not used by another tool in the
    /// project, for a new custom tool on operation `id`.
    fn free_tool_number(&self, id: u32) -> u32 {
        let used: HashSet<u32> = self
            .library_numbers()
            .into_iter()
            .chain(self.project_tools(id).iter().map(|p| p.number))
            .collect();
        (1..).find(|n| !used.contains(n)).expect("a free number")
    }

    /// A custom tool's number must not be a library tool's, nor another, different tool's in
    /// the project: the machine would load the wrong tool for that number.
    fn check_custom_number(&self, op: &Operation) -> Result<(), String> {
        let c = &op.common;
        let n = c.tool_number;
        if self.library_numbers().contains(&n) {
            return Err(format!(
                "T{n} is a tool in the tool library. Give the custom tool a number that isn't in the library."
            ));
        }
        if let Some(tool) = &c.tool
            && let Some(other) = self
                .project_tools(op.id)
                .into_iter()
                .find(|p| p.number == n && !p.tool.same_cutter(tool))
        {
            return Err(format!(
                "T{n} is already used for {} in this project. Give the custom tool another number.",
                other.tool.name
            ));
        }
        Ok(())
    }

    /// If the tool an operation uses has a number that the library on this computer gives to a
    /// different cutter, the message saying so: the machine would load the library's cutter.
    fn differs_from_library(&self, pi: usize, op: &Operation) -> Option<String> {
        let (_, mine) = self.tool_source(pi, op);
        let mine = mine?;
        let lib = self
            .library_tools
            .iter()
            .flatten()
            .find(|p| p.number == mine.number)?;
        if lib.tool.same_cutter(&mine.tool) {
            return None;
        }
        let name = self.doc.active_library.as_deref().unwrap_or("");
        Some(format!(
            "T{} in the tool library \"{name}\" on this computer is a different tool ({}). Pick it again from the library.",
            mine.number, lib.tool.name
        ))
    }

    /// Sets the Machining tool (None = none) as one undo step.
    pub fn set_machining_tool(&mut self, tool: Option<PickedTool>) -> Result<(), String> {
        if let Some(p) = &tool {
            p.check_usable()?;
        }
        self.edit(|doc| doc.machining_tool = tool);
        Ok(())
    }

    /// Sets a part's own tool (None = use the Machining tool) as one undo step.
    pub fn set_part_tool(&mut self, part: usize, tool: Option<PickedTool>) -> Result<(), String> {
        if part >= self.doc.parts.len() {
            return Err(format!("There is no part {part}."));
        }
        if let Some(p) = &tool {
            p.check_usable()?;
        }
        self.edit(|doc| doc.parts[part].tool = tool);
        Ok(())
    }

    /// Where an operation in part `pi` gets its tool, and the tool (None = no tool set).
    fn tool_source(&self, pi: usize, op: &Operation) -> (&'static str, Option<PickedTool>) {
        let c = &op.common;
        if c.tool_source != ToolSource::Inherit {
            return (
                "operation",
                c.tool.clone().map(|tool| PickedTool {
                    number: c.tool_number,
                    tool,
                }),
            );
        }
        if let Some(p) = &self.doc.parts[pi].tool {
            return ("part", Some(p.clone()));
        }
        match &self.doc.machining_tool {
            Some(p) => ("machining", Some(p.clone())),
            None => ("none", None),
        }
    }

    /// The operation with the tool it really uses filled in (from its part or Machining when it
    /// follows the tree), ready to generate. Errors if it follows the tree and no tool is set.
    fn resolve(&self, pi: usize, op: &Operation) -> Result<Operation, String> {
        let (_, picked) = self.tool_source(pi, op);
        let picked = picked.ok_or(NO_TOOL)?;
        let mut op = op.clone();
        op.common.tool_number = picked.number;
        op.common.tool_diameter = picked.tool.diameter;
        op.common.tool = Some(picked.tool);
        Ok(op)
    }

    /// What an operation's toolpath is made from, to tell when it is out of date: the resolved
    /// operation (so a changed Machining or part tool counts), or the operation as it is if no
    /// tool is set. Where the tool came from doesn't count, only its number and diameter: the
    /// toolpath is the same whichever level gave them.
    fn generated_from(&self, pi: usize, op: &Operation) -> Operation {
        match self.resolve(pi, op) {
            Ok(mut r) => {
                r.common.tool_source = ToolSource::Custom;
                r.common.tool = None;
                r
            }
            Err(_) => op.clone(),
        }
    }

    /// Sets the project's machine (by name; None = generic post-processor) as one undo step.
    pub fn set_machine(&mut self, name: Option<String>) {
        if self.doc.machine != name {
            self.edit(|doc| doc.machine = name);
        }
    }

    /// Sets the project's active tool library (by name; None = none) as one undo step.
    pub fn set_active_library(&mut self, name: Option<String>) {
        if self.doc.active_library != name {
            self.edit(|doc| doc.active_library = name);
        }
    }

    /// Sets which shapes an operation machines, as one undo step.
    pub fn set_operation_shapes(&mut self, id: u32, shape_ids: &[u32]) -> Result<(), String> {
        let (pi, oi) = self
            .find_operation(id)
            .ok_or_else(|| format!("There is no operation with id {id}."))?;
        if shape_ids.is_empty() {
            return Err("Select the shapes for the operation first.".into());
        }
        let kind = self.doc.parts[pi].operations[oi].kind.clone();
        self.check_shapes(&kind, shape_ids)?;
        self.edit(|doc| doc.parts[pi].operations[oi].shape_ids = shape_ids.to_vec());
        Ok(())
    }

    /// Deletes an operation as one undo step. Returns false if there is none with that id.
    pub fn delete_operation(&mut self, id: u32) -> bool {
        let Some((pi, oi)) = self.find_operation(id) else {
            return false;
        };
        self.edit(|doc| {
            doc.parts[pi].operations.remove(oi);
        });
        true
    }

    /// The operation's shapes as they are now (None for ones that no longer exist).
    fn current_shapes(&self, op: &Operation) -> Vec<Option<Shape>> {
        op.shape_ids
            .iter()
            .map(|id| {
                self.doc
                    .objects()
                    .find(|o| o.id == *id)
                    .map(|o| o.shape.clone())
            })
            .collect()
    }

    fn make_toolpath(&self, op: &Operation) -> Result<Toolpath, String> {
        op.validate()?;
        let tp = match &op.kind {
            OperationKind::Profile(p) => {
                let paths = self.profile_paths(&op.shape_ids)?;
                profile::profile_toolpath(op.id, &paths, &op.common, p)?
            }
            OperationKind::Pocket(p) => {
                let outlines = self.outlines(&op.shape_ids)?;
                pocket::pocket_toolpath(op.id, &outlines, &op.common, p)?
            }
            OperationKind::Drill(d) => {
                let holes = self.drill_holes(&op.shape_ids)?;
                drill::drill_toolpath(op.id, &holes, &op.common, d)?
            }
        };
        let problems = safety::check(&tp.moves, &tp.limits);
        if !problems.is_empty() {
            return Err(format!(
                "The safety checker rejected this toolpath, so it isn't used. Please report this. {}",
                problems.join(" ")
            ));
        }
        Ok(tp)
    }

    /// Generates the toolpaths of the given operations (all enabled ones if `ids` is empty).
    pub fn generate(&mut self, ids: &[u32]) -> GenerateReport {
        let ops: Vec<(usize, Operation)> = self
            .doc
            .parts
            .iter()
            .enumerate()
            .flat_map(|(pi, p)| p.operations.iter().map(move |o| (pi, o)))
            .filter(|(_, o)| {
                if ids.is_empty() {
                    o.enabled
                } else {
                    ids.contains(&o.id)
                }
            })
            .map(|(pi, o)| (pi, o.clone()))
            .collect();
        let mut report = GenerateReport {
            generated: 0,
            errors: Vec::new(),
            warnings: Vec::new(),
        };
        if self.library_tools.is_none()
            && let Some(name) = &self.doc.active_library
        {
            report.warnings.push(format!(
                "Tool library \"{name}\" isn't on this computer: tool numbers couldn't be checked against it."
            ));
        }
        // One tool number must mean one tool across the whole job (every enabled operation,
        // as in the G-code): the machine loads tools by number.
        let job: Vec<(u32, PickedTool)> = self
            .doc
            .parts
            .iter()
            .enumerate()
            .flat_map(|(pi, p)| p.operations.iter().map(move |o| (pi, o)))
            .filter(|(_, o)| o.enabled)
            .filter_map(|(pi, o)| Some((o.id, self.tool_source(pi, o).1?)))
            .collect();
        let clashes: HashMap<u32, String> = job
            .iter()
            .filter_map(|(id, mine)| {
                let n = mine.number;
                job.iter()
                    .find(|(i, p)| i != id && p.number == n && !p.tool.same_cutter(&mine.tool))
                    .map(|(i, p)| {
                        let other = self.operation(*i).map_or("another operation", |o| &o.name);
                        let msg = format!(
                            "T{n} is used for two different tools: {} here and {} in {other}. Give one of them another number.",
                            mine.tool.name, p.tool.name
                        );
                        (*id, msg)
                    })
            })
            .collect();
        let library_numbers = self.library_numbers();
        for (pi, op) in ops {
            // Generated with the tool it really uses (its own, the part's or Machining's).
            let custom_in_library = op.common.tool_source == ToolSource::Custom
                && library_numbers.contains(&op.common.tool_number);
            let result = if custom_in_library {
                Err(format!(
                    "T{} is a tool in the tool library. Give the custom tool a number that isn't in the library.",
                    op.common.tool_number
                ))
            } else if let Some(e) = self.differs_from_library(pi, &op) {
                Err(e)
            } else if let Some(e) = clashes.get(&op.id) {
                Err(e.clone())
            } else {
                self.resolve(pi, &op)
                    .and_then(|resolved| self.make_toolpath(&resolved))
            };
            match &result {
                Ok(_) => report.generated += 1,
                Err(e) => report.errors.push((op.name.clone(), e.clone())),
            }
            let shapes = self.current_shapes(&op);
            let operation = self.generated_from(pi, &op);
            let tool = self.tool_source(pi, &op).1.map(|p| p.tool);
            self.toolpaths.insert(
                op.id,
                Generated {
                    operation,
                    tool,
                    shapes,
                    result,
                },
            );
        }
        report
    }

    /// The toolpath state of every operation, for the tree.
    pub fn operation_states(&self) -> Vec<OperationState> {
        self.doc
            .parts
            .iter()
            .enumerate()
            .flat_map(|(pi, p)| p.operations.iter().map(move |o| (pi, o)))
            .map(|(pi, op)| {
                let (status, message) = match self.toolpaths.get(&op.id) {
                    None => ("none", None),
                    Some(g) => {
                        let current = g.operation == self.generated_from(pi, op)
                            && g.shapes == self.current_shapes(op);
                        match (&g.result, current) {
                            (Ok(_), true) => ("ok", None),
                            (Err(e), true) => ("error", Some(e.clone())),
                            (Ok(_), false) => ("outdated", None),
                            (Err(e), false) => ("outdated", Some(e.clone())),
                        }
                    }
                };
                let has_open_shapes = self.current_shapes(op).iter().flatten().any(|shape| {
                    matches!(shape, Shape::Line { .. } | Shape::Arc { .. })
                        || matches!(shape, Shape::Polyline(pl) if !pl.closed)
                });
                let (tool_from, picked) = self.tool_source(pi, op);
                OperationState {
                    id: op.id,
                    status: status.into(),
                    message,
                    has_open_shapes,
                    tool_from: tool_from.into(),
                    tool_number: picked.as_ref().map(|p| p.number),
                    tool_diameter: picked.as_ref().map(|p| p.tool.diameter),
                    tool_name: picked.map(|p| p.tool.name),
                }
            })
            .collect()
    }

    /// The G-code for the whole job (spec 7): every enabled operation in tree order, written by
    /// the generic post-processor from the toolpaths as generated, then read back and checked.
    /// `date` is when it is made, as the interface shows it. Refuses (with a plain-English
    /// message) if there is nothing to write, if any enabled operation's toolpath is out of date,
    /// failed or was never generated, or if the read-back check finds anything wrong.
    pub fn gcode(&self, date: &str) -> Result<Program, String> {
        let states = self.operation_states();
        let enabled: Vec<&Operation> = self
            .doc
            .parts
            .iter()
            .flat_map(|p| &p.operations)
            .filter(|o| o.enabled)
            .collect();
        if enabled.is_empty() {
            return Err("Nothing to write: add an operation (and make sure it's enabled).".into());
        }
        let not_ready: Vec<String> = enabled
            .iter()
            .filter_map(|op| {
                let st = states.iter().find(|s| s.id == op.id)?;
                let why = match st.status.as_str() {
                    "ok" => return None,
                    "outdated" => "out of date",
                    "error" => "failed",
                    _ => "not generated",
                };
                Some(format!("{} ({why})", op.name))
            })
            .collect();
        if !not_ready.is_empty() {
            return Err(format!(
                "Press Generate toolpaths and check them first: {}.",
                not_ready.join(", ")
            ));
        }
        let mut job = Vec::new();
        for op in enabled {
            let g = self
                .toolpaths
                .get(&op.id)
                .ok_or_else(|| format!("{} has no toolpath.", op.name))?;
            let tp = g.result.as_ref().map_err(|e| format!("{}: {e}", op.name))?;
            let c = &g.operation.common;
            let (kind, depth_increment) = match &g.operation.kind {
                OperationKind::Profile(p) => (
                    format!(
                        "Profile [{}]",
                        match p.side {
                            ProfileSide::Inside => "Inside",
                            ProfileSide::Outside => "Outside",
                            ProfileSide::OnLine => "On line",
                        }
                    ),
                    Some(c.depth_increment),
                ),
                OperationKind::Pocket(_) => ("Pocket".to_string(), Some(c.depth_increment)),
                OperationKind::Drill(d) => (format!("Drill [{:?}]", d.method), None),
            };
            job.push(JobOperation {
                name: op.name.clone(),
                kind,
                tool_number: c.tool_number,
                tool_name: g
                    .tool
                    .as_ref()
                    .map_or_else(|| "custom tool".to_string(), |t| t.name.clone()),
                tool_diameter: c.tool_diameter,
                spindle: c.spindle_direction,
                spindle_speed: c.spindle_speed,
                stock_surface: tp.limits.stock_surface,
                target_depth: tp.limits.target_depth,
                depth_increment,
                fast_plunge_height: c.fast_plunge_height,
                toolpath: tp,
            });
        }
        let format = PostFormat::generic();
        let header = Header {
            project: self.doc.name.clone(),
            date: date.to_string(),
            machine: self.doc.machine.clone(),
            post: "Generic".into(),
        };
        let program = write_program(&header, &job, &format);
        let problems = check_program(&program, &job, format.decimals);
        if !problems.is_empty() {
            let shown: Vec<&str> = problems.iter().take(5).map(String::as_str).collect();
            return Err(format!(
                "The G-code check failed, so no file was written. Please report this. {}",
                shown.join(" ")
            ));
        }
        Ok(program)
    }

    /// The toolpath of an operation, if one has been generated without error.
    pub fn toolpath(&self, id: u32) -> Option<&Toolpath> {
        self.toolpaths.get(&id).and_then(|g| g.result.as_ref().ok())
    }

    /// Toolpaths to draw: generated ones of existing operations that are enabled and shown.
    pub fn toolpaths_display(&self) -> Vec<ToolpathDisplay> {
        self.doc
            .parts
            .iter()
            .flat_map(|p| &p.operations)
            .filter(|op| op.enabled && op.show_toolpath)
            .filter_map(|op| self.toolpath(op.id).map(toolpath_display))
            .collect()
    }

    /// The tool playback of the drawn toolpaths: the same operations as `toolpaths_display`, in
    /// tree order.
    pub fn playback(&self) -> Playback {
        let mut pb = playback(&self.shown_toolpaths());
        // The cutter of each shown operation, as it was when generated.
        pb.tools = self
            .doc
            .parts
            .iter()
            .flat_map(|p| &p.operations)
            .filter(|op| op.enabled && op.show_toolpath)
            .filter_map(|op| {
                let g = self.toolpaths.get(&op.id)?;
                g.result.as_ref().ok()?;
                let tool = g.tool.as_ref()?;
                Some(PlaybackTool {
                    operation_id: op.id,
                    profile: tool.profile()?,
                    flute_length: tool.flute_length,
                })
            })
            .collect();
        pb
    }

    /// The width the cutter sweeps along each step of `playback()` (same steps, same order), so
    /// it can be revealed as the tool plays back.
    pub fn cut_widths(&self) -> Vec<CutWidthStep> {
        cut_width_steps(&self.playback())
    }

    /// Generated toolpaths of enabled, shown operations in tree order, each with the tool
    /// diameter it was made with.
    fn shown_toolpaths(&self) -> Vec<(&Toolpath, f64)> {
        self.doc
            .parts
            .iter()
            .flat_map(|p| &p.operations)
            .filter(|op| op.enabled && op.show_toolpath)
            .filter_map(|op| {
                // The diameter the toolpath was made with (the setting may have changed since).
                let g = self.toolpaths.get(&op.id)?;
                let tp = g.result.as_ref().ok()?;
                Some((tp, g.operation.common.tool_diameter))
            })
            .collect()
    }

    /// Ids of every object in the drawing.
    pub fn all_ids(&self) -> Vec<u32> {
        self.doc.objects().map(|o| o.id).collect()
    }

    /// Ids of every object on a layer (empty if there's no such layer).
    pub fn ids_on_layer(&self, name: &str) -> Vec<u32> {
        self.doc
            .layers
            .iter()
            .filter(|l| l.name == name)
            .flat_map(|l| l.objects.iter().map(|o| o.id))
            .collect()
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
                BoxMode::Window => {
                    s.all_points().next().is_some() && s.all_points().all(|p| r.contains(p))
                }
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

fn path_segments(
    points: &[[f64; 2]],
    closed: bool,
) -> impl Iterator<Item = ([f64; 2], [f64; 2])> + '_ {
    let n = points.len();
    let count = if n < 2 {
        0
    } else if closed {
        n
    } else {
        n - 1
    };
    (0..count).map(move |i| (points[i], points[(i + 1) % n]))
}

/// Every drawn segment: the path (unless it is a set of markers), the extra paths, and the
/// edges of the label's box.
fn segments(s: &DisplayShape) -> Vec<([f64; 2], [f64; 2])> {
    let mut out: Vec<_> = if s.markers {
        Vec::new()
    } else {
        path_segments(&s.points, s.closed).collect()
    };
    for path in &s.extra {
        out.extend(path_segments(path, false));
    }
    if let Some(label) = &s.label {
        out.extend(path_segments(&label.corners(), true).collect::<Vec<_>>());
    }
    out
}

/// Whether `p` is inside a dimension's label box.
fn in_label(s: &DisplayShape, p: [f64; 2]) -> bool {
    let Some(label) = &s.label else {
        return false;
    };
    let c = label.corners();
    // Inside a convex quadrilateral: on the same side of every edge.
    let side =
        |a: [f64; 2], b: [f64; 2]| (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
    (0..4).all(|i| side(c[i], c[(i + 1) % 4]) >= 0.0)
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
    if in_label(s, p) {
        return 0.0;
    }
    let to_points = s
        .points
        .iter()
        .map(|q| (q[0] - x).hypot(q[1] - y))
        .fold(f64::INFINITY, f64::min);
    segments(s)
        .into_iter()
        .map(|(a, b)| distance_to_segment(p, a, b))
        .fold(to_points, f64::min)
}

fn shape_touches_rect(s: &DisplayShape, r: &Rect) -> bool {
    s.all_points().any(|p| r.contains(p))
        || segments(s)
            .into_iter()
            .any(|(a, b)| segment_hits_rect(a, b, r))
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
