//! Machining operations and their settings (spec section 6): Profile and Pocket.

use serde::{Deserialize, Serialize};

/// Spindle direction (spec 6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpindleDirection {
    CW,
    CCW,
    Off,
}

/// Milling direction. With the spindle turning clockwise, Conventional cuts inside walls
/// clockwise and outside walls anticlockwise; Climb is the reverse (as in the reference output).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MillingDirection {
    Conventional,
    Climb,
}

/// Order of cuts across several separate areas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CutOrdering {
    /// Each area all the way down, then the next.
    DepthFirst,
    /// Each depth level across all areas, then the next level.
    LevelFirst,
}

/// Which feed rate the moves between pocket rings use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepoverFeed {
    Plunge,
    Cut,
}

/// Settings common to all operations (spec 6.1). Lengths in mm, feeds in mm/min.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommonSettings {
    pub tool_number: u32,
    pub tool_diameter: f64,
    /// Z of the top of the material.
    pub stock_surface: f64,
    /// Final Z (below the stock surface). `None` until the user sets it.
    pub target_depth: Option<f64>,
    /// Largest depth of one pass.
    pub depth_increment: f64,
    /// Size of the last pass; 0 = all passes equal.
    pub final_depth_increment: f64,
    /// Z for rapid moves between cuts.
    pub clearance_plane: f64,
    /// How far above the last cut level a rapid down move stops before feeding down.
    pub fast_plunge_height: f64,
    pub cut_feedrate: f64,
    pub plunge_feedrate: f64,
    pub spindle_speed: f64,
    pub spindle_direction: SpindleDirection,
    /// Material left on the walls (negative = overcut).
    pub roughing_clearance: f64,
}

impl Default for CommonSettings {
    fn default() -> Self {
        Self {
            tool_number: 0,
            tool_diameter: 6.0,
            stock_surface: 0.0,
            target_depth: None,
            depth_increment: 1.0,
            final_depth_increment: 0.0,
            clearance_plane: 3.0,
            fast_plunge_height: 1.0,
            cut_feedrate: 800.0,
            plunge_feedrate: 300.0,
            spindle_speed: 10000.0,
            spindle_direction: SpindleDirection::CW,
            roughing_clearance: 0.0,
        }
    }
}

/// Pocket settings (spec 6.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PocketSettings {
    /// Distance between rings as a fraction of the tool diameter.
    pub stepover: f64,
    pub stepover_feed: StepoverFeed,
    pub milling_direction: MillingDirection,
    pub cut_ordering: CutOrdering,
}

impl Default for PocketSettings {
    fn default() -> Self {
        Self {
            stepover: 0.4,
            stepover_feed: StepoverFeed::Plunge,
            milling_direction: MillingDirection::Conventional,
            cut_ordering: CutOrdering::DepthFirst,
        }
    }
}

/// Which side of the shape a profile cuts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileSide {
    Inside,
    Outside,
    /// Tool centre on the line (no offset). The only choice for open shapes.
    OnLine,
}

/// Profile settings (spec 6.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileSettings {
    pub side: ProfileSide,
    pub milling_direction: MillingDirection,
    pub cut_ordering: CutOrdering,
    /// How far a closed profile carries on past its start point (mm).
    pub overlap: f64,
}

impl Default for ProfileSettings {
    fn default() -> Self {
        Self {
            side: ProfileSide::Outside,
            milling_direction: MillingDirection::Conventional,
            cut_ordering: CutOrdering::DepthFirst,
            overlap: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OperationKind {
    Profile(ProfileSettings),
    Pocket(PocketSettings),
}

impl OperationKind {
    pub fn type_name(&self) -> &'static str {
        match self {
            OperationKind::Profile(_) => "Profile",
            OperationKind::Pocket(_) => "Pocket",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Operation {
    /// Unique among operations in the document.
    pub id: u32,
    pub name: String,
    /// Included when toolpaths are generated (and later in G-code).
    pub enabled: bool,
    /// Draw its toolpath in the drawing area.
    pub show_toolpath: bool,
    /// The drawing objects it machines.
    pub shape_ids: Vec<u32>,
    pub common: CommonSettings,
    pub kind: OperationKind,
}

impl Operation {
    /// Checks the settings make sense. Errors are plain-English messages.
    pub fn validate(&self) -> Result<(), String> {
        let c = &self.common;
        let ok = |cond: bool, msg: &str| if cond { Ok(()) } else { Err(msg.to_string()) };
        let numbers = [
            c.tool_diameter,
            c.stock_surface,
            c.depth_increment,
            c.final_depth_increment,
            c.clearance_plane,
            c.fast_plunge_height,
            c.cut_feedrate,
            c.plunge_feedrate,
            c.spindle_speed,
            c.roughing_clearance,
        ];
        ok(
            numbers.iter().all(|n| n.is_finite()) && c.target_depth.is_none_or(f64::is_finite),
            "Every setting must be a number.",
        )?;
        ok(
            c.tool_diameter > 0.0,
            "The tool diameter must be more than 0.",
        )?;
        let target = c.target_depth.ok_or("Set the target depth.")?;
        ok(
            target < c.stock_surface,
            "The target depth must be below the stock surface.",
        )?;
        ok(
            c.clearance_plane > c.stock_surface,
            "The clearance plane must be above the stock surface.",
        )?;
        ok(
            c.depth_increment > 0.0,
            "The depth increment must be more than 0.",
        )?;
        ok(
            c.final_depth_increment >= 0.0,
            "The final depth increment can't be negative.",
        )?;
        ok(
            c.fast_plunge_height > 0.0,
            "The fast plunge height must be more than 0.",
        )?;
        ok(
            c.cut_feedrate > 0.0 && c.plunge_feedrate > 0.0,
            "Feed rates must be more than 0.",
        )?;
        ok(
            c.spindle_speed >= 0.0,
            "The spindle speed can't be negative.",
        )?;
        ok(
            c.tool_diameter / 2.0 + c.roughing_clearance > 0.0,
            "The roughing clearance can't overcut by the tool radius or more.",
        )?;
        match &self.kind {
            OperationKind::Profile(p) => ok(
                p.overlap.is_finite() && p.overlap >= 0.0,
                "The overlap can't be negative.",
            ),
            OperationKind::Pocket(p) => ok(
                p.stepover.is_finite() && p.stepover > 0.0 && p.stepover <= 1.0,
                "The stepover must be more than 0 and at most 1 (a fraction of the tool diameter).",
            ),
        }
    }
}
