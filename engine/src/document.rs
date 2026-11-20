//! The project document: layers of drawing objects, and machining parts.

use serde::{Deserialize, Serialize};

use crate::geometry::Shape;
use crate::operation::Operation;
use crate::tooling::PickedTool;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub name: String,
    pub layers: Vec<Layer>,
    pub parts: Vec<Part>,
    /// Drawing grid spacing in mm, used for display and grid snapping.
    #[serde(default = "default_grid_spacing")]
    pub grid_spacing: f64,
    /// The tool library operations pick their tools from (by name; the libraries themselves
    /// belong to the computer, not the project). None = not chosen yet.
    #[serde(default)]
    pub active_library: Option<String>,
    /// The tool every part and operation uses unless it picks its own. None = not chosen.
    #[serde(default)]
    pub machining_tool: Option<PickedTool>,
    /// The machine the G-code is for (by name; machines belong to the computer). None = the
    /// generic post-processor.
    #[serde(default)]
    pub machine: Option<String>,
    /// Dimensions (measure only; not on any layer, never machined or exported).
    #[serde(default)]
    pub dimensions: Vec<crate::dimension::DimensionObject>,
}

/// Grid spacing for new projects (and older project files that don't have one), in mm.
pub const DEFAULT_GRID_SPACING: f64 = 10.0;

fn default_grid_spacing() -> f64 {
    DEFAULT_GRID_SPACING
}

/// Colours given to new layers, in order: strong, clearly different hues that stand out on
/// the dark drawing area. Yellow is left out: it is the "selected" highlight.
pub const LAYER_PALETTE: [&str; 8] = [
    "#ff0000", // red
    "#00ffff", // cyan
    "#00ff00", // green
    "#ff8000", // orange
    "#00b0ff", // sky blue
    "#ff00ff", // magenta
    "#ffffff", // white
    "#80ff00", // lime
];

fn default_layer_color() -> String {
    LAYER_PALETTE[0].to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    /// Display colour for everything on this layer, as "#rrggbb".
    #[serde(default = "default_layer_color")]
    pub color: String,
    pub objects: Vec<DrawingObject>,
}

/// True if `color` is a "#rrggbb" hex colour.
pub fn is_hex_color(color: &str) -> bool {
    color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawingObject {
    /// Unique within the document.
    pub id: u32,
    pub shape: Shape,
}

/// A group of machining operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub name: String,
    /// The part's own tool, used instead of the Machining tool by its operations that follow
    /// the tree. None = use the Machining tool.
    #[serde(default)]
    pub tool: Option<PickedTool>,
    #[serde(default)]
    pub operations: Vec<Operation>,
}

impl Document {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            layers: vec![Layer {
                name: "Default".to_string(),
                color: default_layer_color(),
                objects: Vec::new(),
            }],
            parts: vec![Part {
                name: "Part1".to_string(),
                tool: None,
                operations: Vec::new(),
            }],
            grid_spacing: DEFAULT_GRID_SPACING,
            active_library: None,
            machining_tool: None,
            machine: None,
            dimensions: Vec::new(),
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
                let color = self.next_layer_color();
                self.layers.push(Layer {
                    name: name.to_string(),
                    color,
                    objects: Vec::new(),
                });
                self.layers.len() - 1
            }
        }
    }

    /// The first palette colour not used by any layer yet (cycling once all are used).
    fn next_layer_color(&self) -> String {
        LAYER_PALETTE
            .iter()
            .find(|c| !self.layers.iter().any(|l| l.color.eq_ignore_ascii_case(c)))
            .unwrap_or(&LAYER_PALETTE[self.layers.len() % LAYER_PALETTE.len()])
            .to_string()
    }

    /// Highest object id in use (0 if empty).
    /// The highest id used by an object or a dimension (they share one numbering).
    pub fn max_id(&self) -> u32 {
        let objects = self.objects().map(|o| o.id);
        let dimensions = self.dimensions.iter().map(|d| d.id);
        objects.chain(dimensions).max().unwrap_or(0)
    }
}
