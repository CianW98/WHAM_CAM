//! WHAM_CAM engine.
//!
//! Owns everything that affects the machine: geometry, (later) toolpaths, tools,
//! post-processors and G-code. It takes data in and returns data out: no file system,
//! no OS calls, no threads, so it also builds for WebAssembly.

pub mod curve;
pub mod dimension;
pub mod display;
pub mod document;
pub mod draw;
pub mod drill;
pub mod dxf;
pub mod edit;
pub mod gcode_read;
pub mod geometry;
pub mod machine;
pub mod offset;
pub mod operation;
pub mod path;
pub mod pocket;
pub mod post;
pub mod profile;
pub mod project;
pub mod project_file;
pub mod safety;
pub mod tooling;
pub mod toolpath;
pub mod transform;

pub use dimension::{Dimension, DimensionKind};
pub use display::{
    ASSUMED_RAPID_RATE, CutWidthStep, DISPLAY_TOLERANCE, Direction, DisplayData, DisplayShape,
    Playback, PlaybackStep, PlaybackTool, ToolpathDisplay, display_data,
};
pub use document::{DEFAULT_GRID_SPACING, Document, DrawingObject, LAYER_PALETTE, Layer, Part};
pub use draw::{Draft, DrawTool, Pick, PickKind, Snap, SnapKind, SnapOptions};
pub use dxf::{DxfDrawing, DxfError, DxfOptions, DxfUnits, read_dxf};
pub use geometry::{Point, Polyline, Shape, Transform, Vertex};
pub use machine::{Machine, Machines, PostProcessor};
pub use operation::Operation;
pub use project::{
    BoxMode, BreakReport, DimensionClick, DxfInfo, GenerateReport, ImportReport, JoinReport,
    NO_TOOL, OperationState, Project, ToolSetting,
};
pub use tooling::{
    EndType, FluteType, LibraryTool, PickedTool, Tool, ToolDrawing, ToolLibrary, Tooling,
};
pub use transform::{TransformOp, shape_bounds};
