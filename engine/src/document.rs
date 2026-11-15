//! The project document: layers of drawing objects, and machining parts.

use serde::{Deserialize, Serialize};

use crate::geometry::Shape;

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

    /// Index of the layer with this name, creating it if needed.
    pub fn layer_index(&mut self, name: &str) -> usize {
        match self.layers.iter().position(|l| l.name == name) {
            Some(i) => i,
            None => {
                self.layers.push(Layer {
                    name: name.to_string(),
                    objects: Vec::new(),
                });
                self.layers.len() - 1
            }
        }
    }

    /// Highest object id in use (0 if empty).
    pub fn max_id(&self) -> u32 {
        self.objects().map(|o| o.id).max().unwrap_or(0)
    }
}
