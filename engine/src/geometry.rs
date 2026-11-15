//! Basic 2D geometry. All units are millimetres.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance_to(self, other: Point) -> f64 {
        (other.x - self.x).hypot(other.y - self.y)
    }
}

/// A polyline corner. `bulge` describes the segment from this vertex to the next one,
/// the same way DXF and the reference program do: 0 = straight line, otherwise tan(sweep angle / 4).
/// Positive bulge = counter-clockwise arc, negative = clockwise.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vertex {
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub bulge: f64,
}

impl Vertex {
    pub fn line(x: f64, y: f64) -> Self {
        Self { x, y, bulge: 0.0 }
    }

    pub fn point(self) -> Point {
        Point::new(self.x, self.y)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Polyline {
    pub vertices: Vec<Vertex>,
    pub closed: bool,
}

impl Polyline {
    /// Axis-aligned rectangle with its lower-left corner at (x, y), drawn counter-clockwise.
    pub fn rectangle(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            vertices: vec![
                Vertex::line(x, y),
                Vertex::line(x + width, y),
                Vertex::line(x + width, y + height),
                Vertex::line(x, y + height),
            ],
            closed: true,
        }
    }
}

/// A drawing shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Shape {
    Polyline(Polyline),
    Circle { center: Point, radius: f64 },
}

impl Shape {
    /// Name shown in the object tree, matching the reference program's wording.
    pub fn type_name(&self) -> &'static str {
        match self {
            Shape::Polyline(_) => "Polyline",
            Shape::Circle { .. } => "Circle",
        }
    }
}
