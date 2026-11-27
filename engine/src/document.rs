//! The project document: layers of drawing objects, and machining parts.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::geometry::Shape;
use crate::operation::{Operation, SharedSetting, SharedSettings};
use crate::tooling::PickedTool;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub name: String,
    pub layers: Vec<Layer>,
    pub parts: Vec<Part>,
    /// Drawing grid spacing in mm, used for display and grid snapping.
    #[serde(default = "default_grid_spacing")]
    pub grid_spacing: f64,
    /// Spacing of the heavier grid lines in mm (display only).
    #[serde(default = "default_major_grid_spacing")]
    pub major_grid_spacing: f64,
    /// The tool library operations pick their tools from (by name; the libraries themselves
    /// belong to the computer, not the project). None = not chosen yet.
    #[serde(default)]
    pub active_library: Option<String>,
    /// The tool every part and operation uses unless it picks its own. None = not chosen.
    #[serde(default)]
    pub machining_tool: Option<PickedTool>,
    /// The shared settings every part and operation follows unless it sets its own (spec 5.4).
    pub machining_settings: SharedSettings,
    /// The machine the G-code is for (by name; machines belong to the computer).
    #[serde(default = "default_source")]
    pub machine: String,
    /// The project's copy of its machine's settings, so the file is complete on its own (spec
    /// 5.8). Only edited in Setup.
    #[serde(default)]
    pub machine_settings: crate::machine::MachineSettings,
    /// The project's machine settings were kept when opened (they differed from the machine on
    /// this computer), so they don't follow the machine until chosen again.
    #[serde(default)]
    pub machine_kept: bool,
    /// Dimensions (measure only; not on any layer, never machined or exported).
    #[serde(default)]
    pub dimensions: Vec<crate::dimension::DimensionObject>,
    /// The area the drawing grid covers (display only).
    #[serde(default)]
    pub grid_area: GridArea,
    /// Which snaps are on while drawing.
    #[serde(default)]
    pub snap: crate::draw::SnapOptions,
    /// The project template the grid and snap settings are copied from (by name; the templates
    /// belong to the computer).
    #[serde(default = "default_source")]
    pub template: String,
    /// The template settings changed in this project: they don't follow the template.
    #[serde(default)]
    pub template_overrides: std::collections::BTreeSet<crate::project_template::TemplateSetting>,
}

/// The built-in machine and template are both called "Default".
fn default_source() -> String {
    "Default".to_string()
}

/// The area the drawing grid is drawn over, in mm. Interface only: it never affects snapping,
/// toolpaths or G-code.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GridArea {
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}

impl Default for GridArea {
    fn default() -> Self {
        Self {
            x_min: -1000.0,
            x_max: 1000.0,
            y_min: -1000.0,
            y_max: 1000.0,
        }
    }
}

impl GridArea {
    /// Checks every value is a number and each minimum is below its maximum.
    pub fn validate(&self) -> Result<(), String> {
        let all = [self.x_min, self.x_max, self.y_min, self.y_max];
        if !all.iter().all(|v| v.is_finite()) {
            return Err("The grid area values must be numbers.".into());
        }
        if self.x_min >= self.x_max {
            return Err("The grid X min must be less than X max.".into());
        }
        if self.y_min >= self.y_max {
            return Err("The grid Y min must be less than Y max.".into());
        }
        Ok(())
    }
}

/// Checks a grid spacing is a number more than 0.
pub fn check_grid_spacing(mm: f64) -> Result<(), String> {
    if mm.is_finite() && mm > 0.0 {
        Ok(())
    } else {
        Err("The grid spacing must be more than 0 mm.".into())
    }
}

/// Checks a major grid spacing is a number more than 0.
pub fn check_major_grid_spacing(mm: f64) -> Result<(), String> {
    if mm.is_finite() && mm > 0.0 {
        Ok(())
    } else {
        Err("The major grid spacing must be more than 0 mm.".into())
    }
}

/// Grid spacing for new projects (and older project files that don't have one), in mm.
pub const DEFAULT_GRID_SPACING: f64 = 10.0;

fn default_grid_spacing() -> f64 {
    DEFAULT_GRID_SPACING
}

/// Major grid spacing for new projects (and project files that don't have one), in mm.
pub const DEFAULT_MAJOR_GRID_SPACING: f64 = 50.0;

fn default_major_grid_spacing() -> f64 {
    DEFAULT_MAJOR_GRID_SPACING
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
    /// The shared settings as the part's operations follow them: its own values for the
    /// settings in `own`, the Machining values for the rest (kept up to date by the project).
    pub settings: SharedSettings,
    /// The shared settings set on the part itself.
    pub own: BTreeSet<SharedSetting>,
    #[serde(default)]
    pub operations: Vec<Operation>,
}

impl Part {
    /// A new part that follows Machining in everything.
    pub fn new(name: &str, machining: &SharedSettings) -> Self {
        Self {
            name: name.to_string(),
            tool: None,
            settings: machining.clone(),
            own: BTreeSet::new(),
            operations: Vec::new(),
        }
    }
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
            parts: vec![Part::new("Part1", &SharedSettings::default())],
            grid_spacing: DEFAULT_GRID_SPACING,
            major_grid_spacing: DEFAULT_MAJOR_GRID_SPACING,
            active_library: None,
            machining_tool: None,
            machining_settings: SharedSettings::default(),
            machine: default_source(),
            machine_settings: crate::machine::MachineSettings::default(),
            machine_kept: false,
            dimensions: Vec::new(),
            grid_area: GridArea::default(),
            snap: crate::draw::SnapOptions::default(),
            template: default_source(),
            template_overrides: Default::default(),
        }
    }

    /// Fills in every shared setting a part or operation doesn't set itself from the level
    /// above: Machining → part → operation (spec 5.4).
    pub fn apply_inheritance(&mut self) {
        let machining = &self.machining_settings;
        for part in &mut self.parts {
            part.settings.follow(machining, &part.own);
            for op in &mut part.operations {
                op.follow(&part.settings);
            }
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
