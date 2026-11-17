//! WHAM_CAM engine.
//!
//! Owns everything that affects the machine: geometry, (later) toolpaths, tools,
//! post-processors and G-code. It takes data in and returns data out: no file system,
//! no OS calls, no threads, so it also builds for WebAssembly.

pub mod display;
pub mod document;
pub mod draw;
pub mod drill;
pub mod dxf;
pub mod edit;
pub mod geometry;
pub mod offset;
pub mod operation;
pub mod path;
pub mod pocket;
pub mod profile;
pub mod project;
pub mod project_file;
pub mod safety;
pub mod toolpath;
pub mod transform;

pub use display::{
    ASSUMED_RAPID_RATE, CutWidthStep, DISPLAY_TOLERANCE, Direction, DisplayData, DisplayShape,
    Playback, PlaybackStep, ToolpathDisplay, display_data,
};
pub use document::{DEFAULT_GRID_SPACING, Document, DrawingObject, LAYER_PALETTE, Layer, Part};
pub use draw::{Draft, DrawTool, Snap, SnapKind, SnapOptions};
pub use dxf::{DxfDrawing, DxfError, DxfUnits, read_dxf};
pub use geometry::{Point, Polyline, Shape, Spline, Transform, Vertex};
pub use operation::Operation;
pub use project::{
    BoxMode, BreakReport, DxfInfo, GenerateReport, ImportReport, JoinReport, OperationState,
    Project,
};
pub use transform::{TransformOp, shape_bounds};
