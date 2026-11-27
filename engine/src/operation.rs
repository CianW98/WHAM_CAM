//! Machining operations and their settings (spec section 6): Profile, Pocket and Drill.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::tooling::{EndType, Tool};

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

/// Where an operation's tool comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ToolSource {
    /// The part's tool, or the Machining tool if the part has none (new operations).
    #[default]
    Inherit,
    /// Its own tool, picked from the active tool library.
    Library,
    /// Its own tool with values set in the operation (not from a library).
    Custom,
}

/// Settings common to all operations (spec 6.1). Lengths in mm, feeds in mm/min.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommonSettings {
    pub tool_number: u32,
    pub tool_diameter: f64,
    /// Where the tool comes from. For `Inherit` the project fills in the tool fields before
    /// generating; for `Library` and `Custom`, `tool` holds the operation's own tool and
    /// `tool_diameter` follows its diameter.
    #[serde(default)]
    pub tool_source: ToolSource,
    /// The operation's own tool (`Library`: a copy of the library tool, so the project keeps it
    /// on a computer with different tooling; `Custom`: the user's own values). Only
    /// `tool_number` and `tool_diameter` are used for toolpaths.
    #[serde(default)]
    pub tool: Option<Tool>,
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
            tool_source: ToolSource::Inherit,
            tool: None,
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

/// One of the settings set on Machining that a part or an operation can override (spec 5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SharedSetting {
    StockSurface,
    DepthIncrement,
    FinalDepthIncrement,
    ClearancePlane,
    FastPlungeHeight,
    CutFeedrate,
    PlungeFeedrate,
    SpindleSpeed,
    SpindleDirection,
    RoughingClearance,
    MillingDirection,
}

impl SharedSetting {
    pub const ALL: [SharedSetting; 11] = [
        SharedSetting::StockSurface,
        SharedSetting::DepthIncrement,
        SharedSetting::FinalDepthIncrement,
        SharedSetting::ClearancePlane,
        SharedSetting::FastPlungeHeight,
        SharedSetting::CutFeedrate,
        SharedSetting::PlungeFeedrate,
        SharedSetting::SpindleSpeed,
        SharedSetting::SpindleDirection,
        SharedSetting::RoughingClearance,
        SharedSetting::MillingDirection,
    ];
}

/// The values of the shared settings (spec 5.4, 6.1): Machining holds them all, and each part
/// and operation holds the values it really uses (its own, or followed from the level above).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedSettings {
    pub stock_surface: f64,
    pub depth_increment: f64,
    pub final_depth_increment: f64,
    pub clearance_plane: f64,
    pub fast_plunge_height: f64,
    pub cut_feedrate: f64,
    pub plunge_feedrate: f64,
    pub spindle_speed: f64,
    pub spindle_direction: SpindleDirection,
    pub roughing_clearance: f64,
    pub milling_direction: MillingDirection,
}

impl Default for SharedSettings {
    fn default() -> Self {
        let c = CommonSettings::default();
        Self {
            stock_surface: c.stock_surface,
            depth_increment: c.depth_increment,
            final_depth_increment: c.final_depth_increment,
            clearance_plane: c.clearance_plane,
            fast_plunge_height: c.fast_plunge_height,
            cut_feedrate: c.cut_feedrate,
            plunge_feedrate: c.plunge_feedrate,
            spindle_speed: c.spindle_speed,
            spindle_direction: c.spindle_direction,
            roughing_clearance: c.roughing_clearance,
            milling_direction: MillingDirection::Conventional,
        }
    }
}

impl SharedSettings {
    /// Copies one setting's value from `from`.
    pub fn copy(&mut self, from: &SharedSettings, s: SharedSetting) {
        match s {
            SharedSetting::StockSurface => self.stock_surface = from.stock_surface,
            SharedSetting::DepthIncrement => self.depth_increment = from.depth_increment,
            SharedSetting::FinalDepthIncrement => {
                self.final_depth_increment = from.final_depth_increment
            }
            SharedSetting::ClearancePlane => self.clearance_plane = from.clearance_plane,
            SharedSetting::FastPlungeHeight => self.fast_plunge_height = from.fast_plunge_height,
            SharedSetting::CutFeedrate => self.cut_feedrate = from.cut_feedrate,
            SharedSetting::PlungeFeedrate => self.plunge_feedrate = from.plunge_feedrate,
            SharedSetting::SpindleSpeed => self.spindle_speed = from.spindle_speed,
            SharedSetting::SpindleDirection => self.spindle_direction = from.spindle_direction,
            SharedSetting::RoughingClearance => self.roughing_clearance = from.roughing_clearance,
            SharedSetting::MillingDirection => self.milling_direction = from.milling_direction,
        }
    }

    /// True if this setting's value differs from `other`'s.
    pub fn differs(&self, other: &SharedSettings, s: SharedSetting) -> bool {
        let mut copy = self.clone();
        copy.copy(other, s);
        copy != *self
    }

    /// The settings `own` doesn't list take their values from `parent`.
    pub fn follow(&mut self, parent: &SharedSettings, own: &BTreeSet<SharedSetting>) {
        for s in SharedSetting::ALL {
            if !own.contains(&s) {
                self.copy(parent, s);
            }
        }
    }

    /// Checks the values make sense on their own. Checks that need the tool or the target depth
    /// happen when generating ([`Operation::validate`]).
    pub fn check(&self) -> Result<(), String> {
        let ok = |cond: bool, msg: &str| if cond { Ok(()) } else { Err(msg.to_string()) };
        ok(
            [
                self.stock_surface,
                self.depth_increment,
                self.final_depth_increment,
                self.clearance_plane,
                self.fast_plunge_height,
                self.cut_feedrate,
                self.plunge_feedrate,
                self.spindle_speed,
                self.roughing_clearance,
            ]
            .iter()
            .all(|n| n.is_finite()),
            "Every setting must be a number.",
        )?;
        ok(
            self.clearance_plane > self.stock_surface,
            "The clearance plane must be above the stock surface.",
        )?;
        ok(
            self.depth_increment > 0.0,
            "The depth increment must be more than 0.",
        )?;
        ok(
            self.final_depth_increment >= 0.0,
            "The final depth increment can't be negative.",
        )?;
        ok(
            self.fast_plunge_height > 0.0,
            "The fast plunge height must be more than 0.",
        )?;
        ok(
            self.cut_feedrate > 0.0 && self.plunge_feedrate > 0.0,
            "Feed rates must be more than 0.",
        )?;
        ok(
            self.spindle_speed >= 0.0,
            "The spindle speed can't be negative.",
        )
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
}

impl Default for ProfileSettings {
    fn default() -> Self {
        Self {
            side: ProfileSide::Outside,
            milling_direction: MillingDirection::Conventional,
            cut_ordering: CutOrdering::DepthFirst,
        }
    }
}

/// How each hole is drilled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrillMethod {
    /// Straight down to the target depth in one go, then up.
    Standard,
    /// Down in pecks of `peck_distance`, back up to the retract height after each to clear chips.
    Peck,
    /// A helix round the hole (hole bigger than the tool), `depth_increment` down per turn, then
    /// a flat circle at the bottom.
    Helical,
}

/// Drill settings (spec 6.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrillSettings {
    pub method: DrillMethod,
    /// Z the tool rapids down to before feeding into a hole, and goes back to between pecks.
    /// Between the stock surface and the clearance plane.
    pub retract_height: f64,
    /// Depth of each peck (Peck only).
    pub peck_distance: f64,
    /// Diameter of the hole (Helical only) for point lists and arcs; more than the tool diameter.
    /// Circles are cut to their own diameter.
    pub hole_diameter: f64,
    /// Direction round the hole (Helical only). Conventional = clockwise (an inside wall).
    pub milling_direction: MillingDirection,
}

impl Default for DrillSettings {
    fn default() -> Self {
        Self {
            method: DrillMethod::Standard,
            retract_height: 3.0,
            peck_distance: 1.0,
            hole_diameter: 10.0,
            milling_direction: MillingDirection::Conventional,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OperationKind {
    Profile(ProfileSettings),
    Pocket(PocketSettings),
    Drill(DrillSettings),
}

impl OperationKind {
    pub fn type_name(&self) -> &'static str {
        match self {
            OperationKind::Profile(_) => "Profile",
            OperationKind::Pocket(_) => "Pocket",
            OperationKind::Drill(_) => "Drill",
        }
    }

    pub fn milling_direction(&self) -> MillingDirection {
        match self {
            OperationKind::Profile(k) => k.milling_direction,
            OperationKind::Pocket(k) => k.milling_direction,
            OperationKind::Drill(k) => k.milling_direction,
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
    /// The shared settings set on this operation itself. The others follow its part (spec 5.4):
    /// the project keeps their values in `common` and `kind` up to date.
    pub own: BTreeSet<SharedSetting>,
}

impl Operation {
    /// The shared settings' values as this operation uses them.
    pub fn shared(&self) -> SharedSettings {
        let c = &self.common;
        SharedSettings {
            stock_surface: c.stock_surface,
            depth_increment: c.depth_increment,
            final_depth_increment: c.final_depth_increment,
            clearance_plane: c.clearance_plane,
            fast_plunge_height: c.fast_plunge_height,
            cut_feedrate: c.cut_feedrate,
            plunge_feedrate: c.plunge_feedrate,
            spindle_speed: c.spindle_speed,
            spindle_direction: c.spindle_direction,
            roughing_clearance: c.roughing_clearance,
            milling_direction: self.kind.milling_direction(),
        }
    }

    /// Writes the shared settings' values into the operation.
    pub fn set_shared(&mut self, s: &SharedSettings) {
        let c = &mut self.common;
        c.stock_surface = s.stock_surface;
        c.depth_increment = s.depth_increment;
        c.final_depth_increment = s.final_depth_increment;
        c.clearance_plane = s.clearance_plane;
        c.fast_plunge_height = s.fast_plunge_height;
        c.cut_feedrate = s.cut_feedrate;
        c.plunge_feedrate = s.plunge_feedrate;
        c.spindle_speed = s.spindle_speed;
        c.spindle_direction = s.spindle_direction;
        c.roughing_clearance = s.roughing_clearance;
        match &mut self.kind {
            OperationKind::Profile(k) => k.milling_direction = s.milling_direction,
            OperationKind::Pocket(k) => k.milling_direction = s.milling_direction,
            OperationKind::Drill(k) => k.milling_direction = s.milling_direction,
        }
    }

    /// The shared settings not in `own` take their values from `parent` (the part's values).
    pub fn follow(&mut self, parent: &SharedSettings) {
        let mut s = self.shared();
        s.follow(parent, &self.own);
        if s != self.shared() {
            self.set_shared(&s);
        }
    }

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
        if c.tool_source == ToolSource::Custom
            && let Some(tool) = &c.tool
        {
            tool.validate()?;
        }
        if let Some(tool) = &c.tool {
            ok(
                tool.end_type == EndType::Flat,
                &format!(
                    "{} is a {}: v1 toolpaths are for end mills only.",
                    tool.name,
                    tool.end_type.label()
                ),
            )?;
        }
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
            OperationKind::Profile(_) => Ok(()),
            OperationKind::Pocket(p) => ok(
                p.stepover.is_finite() && p.stepover > 0.0 && p.stepover <= 1.0,
                "The stepover must be more than 0 and at most 1 (a fraction of the tool diameter).",
            ),
            OperationKind::Drill(d) => {
                ok(
                    [d.retract_height, d.peck_distance, d.hole_diameter]
                        .iter()
                        .all(|n| n.is_finite()),
                    "Every setting must be a number.",
                )?;
                ok(
                    d.retract_height > c.stock_surface && d.retract_height <= c.clearance_plane,
                    "The retract height must be above the stock surface and not above the clearance plane.",
                )?;
                match d.method {
                    DrillMethod::Standard => Ok(()),
                    DrillMethod::Peck => ok(
                        d.peck_distance > 0.0,
                        "The peck distance must be more than 0.",
                    ),
                    // Checked per hole when generating: circles use their own diameter.
                    DrillMethod::Helical => ok(
                        d.hole_diameter > 0.0,
                        "The hole diameter must be more than 0.",
                    ),
                }
            }
        }
    }
}
