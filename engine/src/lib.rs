//! WHAM_CAM engine.
//!
//! Owns everything that affects the machine: geometry, (later) toolpaths, tools,
//! post-processors and G-code. It takes data in and returns data out: no file system,
//! no OS calls, no threads, so it also builds for WebAssembly.

pub mod display;
pub mod document;
pub mod draw;
pub mod dxf;
pub mod geometry;
pub mod project;
pub mod transform;

pub use display::{DISPLAY_TOLERANCE, Direction, DisplayData, DisplayShape, display_data};
pub use document::{DEFAULT_GRID_SPACING, Document, DrawingObject, LAYER_PALETTE, Layer, Part};
pub use draw::{Draft, DrawTool, Snap, SnapKind, SnapOptions};
pub use dxf::{DxfDrawing, DxfError, DxfUnits, read_dxf};
pub use geometry::{Point, Polyline, Shape, Spline, Transform, Vertex};
pub use project::{BoxMode, DxfInfo, ImportReport, Project};
pub use transform::{TransformOp, shape_bounds};
