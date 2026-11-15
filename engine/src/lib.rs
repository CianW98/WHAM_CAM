//! WHAM_CAM engine.
//!
//! Owns everything that affects the machine: geometry, (later) toolpaths, tools,
//! post-processors and G-code. It takes data in and returns data out: no file system,
//! no OS calls, no threads, so it also builds for WebAssembly.

pub mod display;
pub mod document;
pub mod geometry;

pub use display::{DISPLAY_TOLERANCE, DisplayData, DisplayShape, display_data};
pub use document::{Document, DrawingObject, Layer, Part, demo_document};
pub use geometry::{Point, Polyline, Shape, Vertex};
