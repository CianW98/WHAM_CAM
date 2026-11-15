//! WHAM_CAM engine.
//!
//! Owns everything that affects the machine: geometry, (later) toolpaths, tools,
//! post-processors and G-code. It takes data in and returns data out: no file system,
//! no OS calls, no threads, so it also builds for WebAssembly.

pub mod display;
pub mod document;
pub mod dxf;
pub mod geometry;
pub mod project;

pub use display::{DISPLAY_TOLERANCE, DisplayData, DisplayShape, display_data};
pub use document::{Document, DrawingObject, LAYER_PALETTE, Layer, Part};
pub use dxf::{DxfDrawing, DxfError, DxfUnits, read_dxf};
pub use geometry::{Point, Polyline, Shape, Spline, Transform, Vertex};
pub use project::{BoxMode, DxfInfo, ImportReport, Project};
