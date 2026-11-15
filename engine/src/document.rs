//! The project document: layers of drawing objects, and machining parts.

use serde::{Deserialize, Serialize};

use crate::geometry::{Point, Polyline, Shape};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub name: String,
    pub layers: Vec<Layer>,
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub objects: Vec<DrawingObject>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawingObject {
    /// Unique within the document.
    pub id: u32,
    pub shape: Shape,
}

/// A group of machining operations. Operations are added in a later step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub name: String,
}

impl Document {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            layers: vec![Layer {
                name: "Default".to_string(),
                objects: Vec::new(),
            }],
            parts: vec![Part {
                name: "Part1".to_string(),
            }],
        }
    }

    /// Every drawing object in the document, across all layers.
    pub fn objects(&self) -> impl Iterator<Item = &DrawingObject> {
        self.layers.iter().flat_map(|layer| layer.objects.iter())
    }
}

/// A small demo document used by the skeleton app to prove the engine-to-screen pipeline.
pub fn demo_document() -> Document {
    let mut doc = Document::new("Untitled");
    doc.layers[0].objects = vec![
        DrawingObject {
            id: 1,
            shape: Shape::Polyline(Polyline::rectangle(0.0, 0.0, 100.0, 80.0)),
        },
        DrawingObject {
            id: 2,
            shape: Shape::Circle {
                center: Point::new(50.0, 40.0),
                radius: 25.0,
            },
        },
    ];
    doc
}
