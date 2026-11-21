//! Tooling (spec 5.5): the master tool list and the tool libraries.
//!
//! The **master tool list** holds every physical tool the user has (size and shape only: feeds and
//! speeds depend on the material too, so they stay with the operation in v1). A **tool library**
//! is a set of master tools, each given a tool number, e.g. the tools loaded in one machine's tool
//! changer. A project picks one library as its active library; picking a tool for an operation
//! copies the tool into the operation, so the project still opens and cuts the same on a computer
//! with different tooling.
//!
//! The tooling belongs to the computer, not to a project. The engine only turns it into text and
//! back; the app reads and writes the file.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::geometry::{Point, on_circle};

/// The shape of a tool's cutting end. v1 toolpaths are for end mills (`Flat`) only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EndType {
    Flat,
    Ball,
    VBit,
    Drill,
}

impl EndType {
    pub fn label(self) -> &'static str {
        match self {
            EndType::Flat => "end mill",
            EndType::Ball => "ball nose",
            EndType::VBit => "V-bit",
            EndType::Drill => "drill",
        }
    }
}

/// How the flutes run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FluteType {
    /// Upcut (right-hand spiral): lifts chips up out of the cut.
    #[default]
    Upcut,
    /// Straight flutes, along the tool.
    Straight,
    /// Downcut (left-hand spiral): pushes chips down, for a clean top edge.
    Downcut,
}

impl FluteType {
    pub fn label(self) -> &'static str {
        match self {
            FluteType::Upcut => "Upcut",
            FluteType::Straight => "Straight",
            FluteType::Downcut => "Downcut",
        }
    }

    /// Upcut and downcut flutes have a helix angle.
    pub fn has_helix(self) -> bool {
        self != FluteType::Straight
    }
}

/// The most common helix angle for general-purpose end mills and spiral router bits.
pub const DEFAULT_HELIX_ANGLE: f64 = 30.0;

fn default_helix_angle() -> f64 {
    DEFAULT_HELIX_ANGLE
}

fn default_v_angle() -> f64 {
    90.0
}

/// One physical tool. Lengths in mm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tool {
    /// Unique in the master tool list.
    pub id: u32,
    pub name: String,
    pub end_type: EndType,
    pub diameter: f64,
    /// None = the same as the cutting diameter.
    #[serde(default)]
    pub shank_diameter: Option<f64>,
    pub flutes: u32,
    #[serde(default)]
    pub flute_type: FluteType,
    /// Helix (spiral) angle from the tool's axis, in degrees: Upcut and Downcut flutes, not
    /// drills. (Drills have no flute type in v1: `flute_type` is ignored for them.)
    #[serde(default = "default_helix_angle")]
    pub helix_angle: f64,
    pub flute_length: f64,
    pub overall_length: f64,
    /// Included angle of the point (V-bit only).
    #[serde(default = "default_v_angle")]
    pub v_angle: f64,
    /// The name is made from the tool's values ([`Tool::generated_name`]) and follows them when
    /// they change. False once the user types their own name (and for tools saved before
    /// automatic names existed, so their names stay as they were).
    #[serde(default)]
    pub auto_name: bool,
}

/// Point angle drawn for drills (the common 118°); drills have no angle setting in v1.
const DRILL_POINT_ANGLE: f64 = 118.0;

/// A number for a tool name: up to 3 decimals, trailing zeros trimmed (6, 3.175, 0.5).
fn short(n: f64) -> String {
    let s = format!("{n:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The side view of a tool for the tool form's picture, tip down at (0, 0), in mm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDrawing {
    /// The tool's silhouette, closed (last point joins the first).
    pub outline: Vec<Point>,
    /// Height where the flutes end (the shank starts).
    pub flute_top: f64,
    /// Radius of the flutes at `flute_top` (to draw the line between flutes and shank).
    pub flute_top_radius: f64,
    /// The cutting edges seen on the front of the flutes, as lines (each a list of points):
    /// helices for upcut and downcut flutes, straight lines for straight flutes.
    pub flute_lines: Vec<Vec<Point>>,
}

impl Tool {
    /// The automatic name: diameter, (V angle,) flutes, flute type, end type and flute length,
    /// e.g. "6mm 2F Upcut End Mill, 20mm flute" or "6mm 90° 2F Straight V-Bit, 10mm flute".
    /// The helix angle is not part of the name, nor the flute type for drills
    /// ("6mm 2F Drill, 20mm flute").
    pub fn generated_name(&self) -> String {
        let kind = match self.end_type {
            EndType::Flat => "End Mill",
            EndType::Ball => "Ball Nose",
            EndType::VBit => "V-Bit",
            EndType::Drill => "Drill",
        };
        let angle = if self.end_type == EndType::VBit {
            format!(" {}°", short(self.v_angle))
        } else {
            String::new()
        };
        // Drills have no flute type in v1.
        let flute = if self.end_type == EndType::Drill {
            String::new()
        } else {
            format!("{} ", self.flute_type.label())
        };
        format!(
            "{}mm{angle} {}F {flute}{kind}, {}mm flute",
            short(self.diameter),
            self.flutes,
            short(self.flute_length)
        )
    }

    /// The tool with its automatic name filled in (if it uses one) and its name trimmed.
    pub fn named(mut self) -> Self {
        self.name = if self.auto_name {
            self.generated_name()
        } else {
            self.name.trim().to_string()
        };
        self
    }

    /// The cutting edges on the front half of the flutes between heights `y0` and `y1`, seen
    /// from the side. Edge k sits at angle 2πk/n round the tool (plus a turn along the helix);
    /// on the front half (cos > 0) it shows at x = r·sin(angle). A right-hand spiral rises to the
    /// left (like a backslash), a downcut to the right ("/"). Drills are drawn as twist drills: a
    /// spiral at the common 30° helix, whatever their stored flute type.
    fn flute_edges(&self, r: f64, y0: f64, y1: f64) -> Vec<Vec<Point>> {
        use std::f64::consts::{PI, TAU};
        let n = self.flutes.max(1);
        let helix = if self.end_type == EndType::Drill {
            DEFAULT_HELIX_ANGLE
        } else {
            self.helix_angle
        };
        // Axial length of one full turn of a flute.
        // Drills have no flute type in v1: always drawn as a twist drill (spiral).
        let flute_type = if self.end_type == EndType::Drill {
            FluteType::Upcut
        } else {
            self.flute_type
        };
        let lead = match flute_type {
            FluteType::Straight => f64::INFINITY,
            _ => TAU * r / helix.to_radians().tan(),
        };
        let hand = if flute_type == FluteType::Downcut {
            1.0
        } else {
            -1.0
        };
        const STEPS: u32 = 240;
        let mut lines = Vec::new();
        for k in 0..n {
            // Offset by a quarter pitch so straight flutes don't sit on the outline.
            let start = TAU * f64::from(k) / f64::from(n) + PI / (2.0 * f64::from(n));
            let mut run: Vec<Point> = Vec::new();
            for i in 0..=STEPS {
                let y = y0 + (y1 - y0) * f64::from(i) / f64::from(STEPS);
                let angle = start + TAU * (y - y0) / lead;
                if angle.cos() > 0.0 {
                    run.push(Point::new(hand * r * angle.sin(), y));
                } else if run.len() > 1 {
                    lines.push(std::mem::take(&mut run));
                } else {
                    run.clear();
                }
            }
            if run.len() > 1 {
                lines.push(run);
            }
        }
        lines
    }

    /// The right-hand half of the tool's side view, tip at (0, 0) up the axis: tip, flutes,
    /// shank, ending at the outside top corner. Also the height where the full diameter starts
    /// and the flute radius at the flute top. None if the values aren't valid.
    fn half_profile(&self) -> Option<(Vec<Point>, f64, f64)> {
        Tool {
            name: "x".into(),
            ..self.clone()
        }
        .validate()
        .ok()?;
        let r = self.diameter / 2.0;
        let s = self.shank() / 2.0;
        let fl = self.flute_length;
        let len = self.overall_length;

        // Right-hand side, from the tip up to the top of the flutes.
        let mut right = vec![Point::new(0.0, 0.0)];
        let point_angle = match self.end_type {
            EndType::VBit => Some(self.v_angle),
            EndType::Drill => Some(DRILL_POINT_ANGLE),
            _ => None,
        };
        // Height where the full diameter starts.
        let mut tip_top = match (self.end_type, point_angle) {
            (EndType::Ball, _) => {
                let centre = Point::new(0.0, r);
                for i in 1..=16 {
                    // A quarter circle on this side, from straight below the centre to level.
                    let a = (-90.0 + 90.0 * f64::from(i) / 16.0_f64).to_radians();
                    right.push(on_circle(centre, r, a));
                }
                r
            }
            (_, Some(angle)) => {
                let half = (angle / 2.0).to_radians().tan();
                let h = r / half;
                if h <= fl {
                    right.push(Point::new(r, h));
                } else {
                    // The point is longer than the flutes: it is cut off at the flute top.
                    right.push(Point::new(fl * half, fl));
                }
                h
            }
            _ => {
                right.push(Point::new(r, 0.0));
                0.0
            }
        };
        tip_top = tip_top.min(fl);
        let top_radius = right.last().map_or(r, |p| p.x);
        if tip_top < fl {
            right.push(Point::new(r, fl));
        }
        right.push(Point::new(s, fl));
        right.push(Point::new(s, len));

        Some((right, tip_top, top_radius))
    }

    /// The tool's outline for a solid model turned round its axis (the 3D playback cutter):
    /// from the tip on the axis, out and up the side, to the top of the shank back on the axis.
    /// x = radius, y = height above the tip, mm. None if the values aren't valid.
    pub fn profile(&self) -> Option<Vec<Point>> {
        let (mut right, _, _) = self.half_profile()?;
        right.push(Point::new(0.0, self.overall_length));
        right.dedup_by(|a, b| a.distance_to(*b) < 1e-9);
        Some(right)
    }

    /// The side view for the tool form's picture. None if the values aren't valid.
    pub fn drawing(&self) -> Option<ToolDrawing> {
        let (right, tip_top, top_radius) = self.half_profile()?;
        let r = self.diameter / 2.0;
        let fl = self.flute_length;

        // Left side: the mirror image, top down.
        let left: Vec<Point> = right.iter().rev().map(|p| Point::new(-p.x, p.y)).collect();
        let mut outline: Vec<Point> = Vec::new();
        for p in right.into_iter().chain(left) {
            if outline.last().is_none_or(|q| q.distance_to(p) > 1e-9) {
                outline.push(p);
            }
        }
        if outline.len() > 1 && outline[0].distance_to(*outline.last().unwrap()) < 1e-9 {
            outline.pop();
        }

        let flute_lines = if tip_top < fl {
            self.flute_edges(r, tip_top, fl)
        } else {
            Vec::new()
        };

        Some(ToolDrawing {
            outline,
            flute_top: fl,
            flute_top_radius: if tip_top < fl { r } else { top_radius },
            flute_lines,
        })
    }

    /// A new tool with sensible starting values, for the Add tool form.
    pub fn new_default() -> Self {
        Self {
            id: 0,
            name: "New tool".into(),
            end_type: EndType::Flat,
            diameter: 6.0,
            shank_diameter: None,
            flutes: 2,
            flute_type: FluteType::Upcut,
            helix_angle: DEFAULT_HELIX_ANGLE,
            flute_length: 20.0,
            overall_length: 60.0,
            v_angle: default_v_angle(),
            auto_name: true,
        }
        .named()
    }

    /// True if the helix angle applies: upcut or downcut flutes, and not a drill.
    pub fn uses_helix_angle(&self) -> bool {
        self.flute_type.has_helix() && self.end_type != EndType::Drill
    }

    /// True if both describe the same cutter: every value the same, ignoring the name and the
    /// master-list id (which differ between computers and don't change the cutter).
    pub fn same_cutter(&self, other: &Tool) -> bool {
        let plain = |t: &Tool| Tool {
            id: 0,
            name: String::new(),
            auto_name: false,
            ..t.clone()
        };
        plain(self) == plain(other)
    }

    /// The shank diameter, or the cutting diameter if it isn't set.
    pub fn shank(&self) -> f64 {
        self.shank_diameter.unwrap_or(self.diameter)
    }

    /// Checks the values make sense. Errors are plain-English messages.
    pub fn validate(&self) -> Result<(), String> {
        let ok = |cond: bool, msg: &str| if cond { Ok(()) } else { Err(msg.to_string()) };
        ok(!self.name.trim().is_empty(), "A tool needs a name.")?;
        let numbers = [
            self.diameter,
            self.shank(),
            self.flute_length,
            self.overall_length,
            self.v_angle,
        ];
        ok(
            numbers.iter().all(|n| n.is_finite()),
            "Every setting must be a number.",
        )?;
        ok(self.diameter > 0.0, "The diameter must be more than 0.")?;
        ok(
            self.shank() > 0.0,
            "The shank diameter must be more than 0.",
        )?;
        ok(self.flutes >= 1, "A tool needs at least 1 flute.")?;
        ok(
            self.flute_length > 0.0,
            "The flute length must be more than 0.",
        )?;
        ok(
            self.overall_length >= self.flute_length,
            "The overall length can't be less than the flute length.",
        )?;
        if self.uses_helix_angle() {
            ok(
                self.helix_angle.is_finite() && self.helix_angle > 0.0 && self.helix_angle < 90.0,
                "The helix angle must be more than 0° and less than 90°.",
            )?;
        }
        if self.end_type == EndType::VBit {
            ok(
                self.v_angle > 0.0 && self.v_angle < 180.0,
                "The V angle must be more than 0° and less than 180°.",
            )?;
        }
        Ok(())
    }
}

/// A tool picked from a tool library for Machining, a part or an operation: its tool number
/// there and a copy of the tool, kept in the project so it opens and cuts the same on a
/// computer with different tooling.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PickedTool {
    pub number: u32,
    pub tool: Tool,
}

impl PickedTool {
    /// Only end mills can cut in v1.
    pub fn check_usable(&self) -> Result<(), String> {
        if self.tool.end_type == EndType::Flat {
            Ok(())
        } else {
            Err(format!(
                "{} is a {}: v1 toolpaths are for end mills only.",
                self.tool.name,
                self.tool.end_type.label()
            ))
        }
    }
}

/// A master tool in a library, with the tool number it has there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryTool {
    /// Tool number (T number), 1 or more, unique in the library.
    pub number: u32,
    /// The master tool's id.
    pub tool_id: u32,
}

/// A named set of numbered tools. All values are mm, whatever tooling it is for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolLibrary {
    /// Unique among the libraries.
    pub name: String,
    /// In tool number order.
    pub tools: Vec<LibraryTool>,
}

/// The master tool list and the tool libraries of this computer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Tooling {
    pub tools: Vec<Tool>,
    pub libraries: Vec<ToolLibrary>,
}

impl Tooling {
    pub fn tool(&self, id: u32) -> Option<&Tool> {
        self.tools.iter().find(|t| t.id == id)
    }

    pub fn library(&self, name: &str) -> Option<&ToolLibrary> {
        self.libraries.iter().find(|l| l.name == name)
    }

    fn library_mut(&mut self, name: &str) -> Result<&mut ToolLibrary, String> {
        self.libraries
            .iter_mut()
            .find(|l| l.name == name)
            .ok_or_else(|| format!("There is no tool library called \"{name}\"."))
    }

    /// Adds a tool to the master list (its `id` is ignored). Returns its new id.
    pub fn add_tool(&mut self, tool: Tool) -> Result<u32, String> {
        let mut tool = tool.named();
        tool.validate()?;
        tool.id = self.tools.iter().map(|t| t.id).max().unwrap_or(0) + 1;
        let id = tool.id;
        self.tools.push(tool);
        Ok(id)
    }

    /// Replaces a master tool's values (matched by id).
    pub fn set_tool(&mut self, tool: Tool) -> Result<(), String> {
        let tool = tool.named();
        tool.validate()?;
        let slot = self
            .tools
            .iter_mut()
            .find(|t| t.id == tool.id)
            .ok_or_else(|| format!("There is no tool with id {}.", tool.id))?;
        *slot = tool;
        Ok(())
    }

    /// The names of the libraries a master tool is in.
    pub fn libraries_using(&self, id: u32) -> Vec<String> {
        self.libraries
            .iter()
            .filter(|l| l.tools.iter().any(|t| t.tool_id == id))
            .map(|l| l.name.clone())
            .collect()
    }

    /// Deletes a master tool and takes it out of every library. False if there is none.
    pub fn delete_tool(&mut self, id: u32) -> bool {
        let before = self.tools.len();
        self.tools.retain(|t| t.id != id);
        if self.tools.len() == before {
            return false;
        }
        for lib in &mut self.libraries {
            lib.tools.retain(|t| t.tool_id != id);
        }
        true
    }

    /// Adds an empty library called "Library 1", "Library 2", ... Returns its name.
    pub fn add_library(&mut self) -> String {
        let name = (1..)
            .map(|n| format!("Library {n}"))
            .find(|n| self.library(n).is_none())
            .expect("there is always a free name");
        self.libraries.push(ToolLibrary {
            name: name.clone(),
            tools: Vec::new(),
        });
        name
    }

    /// Renames a library. Returns the new name as stored (trimmed).
    pub fn rename_library(&mut self, old: &str, new: &str) -> Result<String, String> {
        let new = new.trim();
        if new.is_empty() {
            return Err("A tool library needs a name.".into());
        }
        if new != old && self.library(new).is_some() {
            return Err(format!("There is already a tool library called \"{new}\"."));
        }
        self.library_mut(old)?.name = new.to_string();
        Ok(new.to_string())
    }

    /// Deletes a library (the master tools stay). False if there is none.
    pub fn delete_library(&mut self, name: &str) -> bool {
        let before = self.libraries.len();
        self.libraries.retain(|l| l.name != name);
        self.libraries.len() != before
    }

    /// Adds a master tool to a library as tool `number`.
    pub fn add_library_tool(
        &mut self,
        library: &str,
        tool_id: u32,
        number: u32,
    ) -> Result<(), String> {
        if self.tool(tool_id).is_none() {
            return Err(format!("There is no tool with id {tool_id}."));
        }
        let lib = self.library_mut(library)?;
        check_number_free(lib, number)?;
        lib.tools.push(LibraryTool { number, tool_id });
        lib.tools.sort_by_key(|t| t.number);
        Ok(())
    }

    /// Changes the tool number of a tool in a library.
    pub fn set_library_tool_number(
        &mut self,
        library: &str,
        old: u32,
        new: u32,
    ) -> Result<(), String> {
        let lib = self.library_mut(library)?;
        if old == new {
            return Ok(());
        }
        check_number_free(lib, new)?;
        let entry = lib
            .tools
            .iter_mut()
            .find(|t| t.number == old)
            .ok_or_else(|| format!("There is no tool {old} in \"{library}\"."))?;
        entry.number = new;
        lib.tools.sort_by_key(|t| t.number);
        Ok(())
    }

    /// Takes tool `number` out of a library (the master tool stays). False if there is none.
    pub fn remove_library_tool(&mut self, library: &str, number: u32) -> bool {
        let Ok(lib) = self.library_mut(library) else {
            return false;
        };
        let before = lib.tools.len();
        lib.tools.retain(|t| t.number != number);
        lib.tools.len() != before
    }

    /// The master tool that is tool `number` in a library.
    pub fn library_tool(&self, library: &str, number: u32) -> Option<&Tool> {
        let entry = self
            .library(library)?
            .tools
            .iter()
            .find(|t| t.number == number)?;
        self.tool(entry.tool_id)
    }
}

fn check_number_free(lib: &ToolLibrary, number: u32) -> Result<(), String> {
    if number == 0 {
        return Err("Tool numbers start at 1.".into());
    }
    if lib.tools.iter().any(|t| t.number == number) {
        return Err(format!(
            "\"{}\" already has a tool {number}. Each tool number can only be used once in a library.",
            lib.name
        ));
    }
    Ok(())
}

// --- Tooling file ---------------------------------------------------------------------------------

/// The value of the "format" field, so other JSON files are recognised as not ours.
pub const FORMAT_NAME: &str = "WHAM_CAM tooling";

/// The tooling file format version written by this WHAM_CAM.
pub const FORMAT_VERSION: u32 = 1;

#[derive(Serialize)]
struct FileOut<'a> {
    format: &'a str,
    format_version: u32,
    saved_by: String,
    #[serde(flatten)]
    tooling: &'a Tooling,
}

/// The tooling as file text (pretty-printed JSON).
pub fn to_text(tooling: &Tooling) -> String {
    let file = FileOut {
        format: FORMAT_NAME,
        format_version: FORMAT_VERSION,
        saved_by: format!("WHAM_CAM {}", env!("CARGO_PKG_VERSION")),
        tooling,
    };
    serde_json::to_string_pretty(&file).expect("tooling always converts to JSON") + "\n"
}

/// Reads tooling file text. Errors are plain-English messages saying what is wrong.
pub fn from_text(text: &str) -> Result<Tooling, String> {
    const NOT_OURS: &str = "This isn't a WHAM_CAM tooling file.";
    let value: Value = serde_json::from_str(text).map_err(|_| NOT_OURS.to_string())?;
    if value.get("format").and_then(Value::as_str) != Some(FORMAT_NAME) {
        return Err(NOT_OURS.into());
    }
    let version = value
        .get("format_version")
        .and_then(Value::as_u64)
        .ok_or("The tooling file has no format version; it may be damaged.")?;
    if version > u64::from(FORMAT_VERSION) {
        return Err(
            "The tooling file was made with a newer WHAM_CAM. Please update WHAM_CAM.".into(),
        );
    }
    let tooling: Tooling =
        serde_json::from_value(value).map_err(|e| format!("The tooling file is damaged ({e})."))?;
    check(&tooling)?;
    Ok(tooling)
}

/// Checks tooling read from a file makes sense.
fn check(t: &Tooling) -> Result<(), String> {
    let damaged = |what: String| Err(format!("The tooling file is damaged: {what}."));
    let mut ids = HashSet::new();
    for tool in &t.tools {
        if !ids.insert(tool.id) {
            return damaged(format!("two tools have the id {}", tool.id));
        }
        if let Err(e) = tool.validate() {
            return damaged(format!(
                "tool \"{}\": {}",
                tool.name,
                e.trim_end_matches('.')
            ));
        }
    }
    let mut names = HashSet::new();
    for lib in &t.libraries {
        if lib.name.trim().is_empty() {
            return damaged("a library has no name".into());
        }
        if !names.insert(lib.name.as_str()) {
            return damaged(format!("there are two libraries called \"{}\"", lib.name));
        }
        let mut numbers = HashSet::new();
        for entry in &lib.tools {
            if entry.number == 0 || !numbers.insert(entry.number) {
                return damaged(format!(
                    "library \"{}\" has tool number {} more than once or as 0",
                    lib.name, entry.number
                ));
            }
            if !ids.contains(&entry.tool_id) {
                return damaged(format!(
                    "library \"{}\" uses a tool that isn't in the master list",
                    lib.name
                ));
            }
        }
    }
    Ok(())
}
