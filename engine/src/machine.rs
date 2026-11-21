//! Machines (spec 5.7): the machines G-code is written for, with the post-processor each uses.
//! Like the tooling, the machine list belongs to the computer, not to a project: a project only
//! names its machine. The engine turns the list into text and back; the app reads and writes
//! the file.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What a project's machine choice shows when no machine is chosen (the generic
/// post-processor). No machine may have this name.
pub const GENERIC: &str = "Generic";

/// The post-processor a machine uses. v1 has only the generic one (spec 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PostProcessor {
    #[default]
    Generic,
}

impl PostProcessor {
    pub fn label(self) -> &'static str {
        match self {
            PostProcessor::Generic => "Generic",
        }
    }
}

/// How the machine's spindle is controlled. Stored only: the post-processors of 1.5 will use it
/// (a machine with on/off control gets no spindle speed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SpindleControl {
    /// Speed set by the G-code (S word).
    #[default]
    VariableRpm,
    /// Switched on and off only.
    OnOff,
}

/// The rapid rates of a machine (mm/min), used to time rapids in the tool playback.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RapidRates {
    /// X and Y (each axis).
    pub xy: f64,
    pub z: f64,
}

/// Rapid rate of a new machine, and of the playback with no machine chosen (mm/min).
pub const DEFAULT_RAPID_RATE: f64 = 5000.0;

impl Default for RapidRates {
    fn default() -> Self {
        RapidRates {
            xy: DEFAULT_RAPID_RATE,
            z: DEFAULT_RAPID_RATE,
        }
    }
}

/// How far an axis can travel, in machine coordinates (mm). Stored only for now.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Travel {
    pub min: f64,
    pub max: f64,
}

/// One machine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Machine {
    /// Unique among the machines.
    pub name: String,
    #[serde(default)]
    pub post: PostProcessor,
    #[serde(default)]
    pub rapid: RapidRates,
    /// Has an automatic tool changer. Stored only for now.
    #[serde(default)]
    pub tool_changer: bool,
    /// Travel limits per axis; None = not set. Stored only for now.
    #[serde(default)]
    pub travel_x: Option<Travel>,
    #[serde(default)]
    pub travel_y: Option<Travel>,
    #[serde(default)]
    pub travel_z: Option<Travel>,
    #[serde(default)]
    pub spindle_control: SpindleControl,
    /// None = not set. Stored only for now.
    #[serde(default)]
    pub max_spindle_rpm: Option<f64>,
    /// Highest cutting feed rate (mm/min); None = not set. Stored only for now.
    #[serde(default)]
    pub max_feed: Option<f64>,
}

impl Machine {
    /// A machine with the default settings.
    pub fn new(name: &str) -> Machine {
        Machine {
            name: name.to_string(),
            post: PostProcessor::Generic,
            rapid: RapidRates::default(),
            tool_changer: false,
            travel_x: None,
            travel_y: None,
            travel_z: None,
            spindle_control: SpindleControl::VariableRpm,
            max_spindle_rpm: None,
            max_feed: None,
        }
    }

    /// Checks the settings, saying in plain English what is wrong.
    pub fn check(&self) -> Result<(), String> {
        let positive = |v: f64| v.is_finite() && v > 0.0;
        if !positive(self.rapid.xy) || !positive(self.rapid.z) {
            return Err("Rapid rates must be more than 0 mm/min.".into());
        }
        for (axis, travel) in [
            ("X", self.travel_x),
            ("Y", self.travel_y),
            ("Z", self.travel_z),
        ] {
            if let Some(t) = travel
                && !(t.min.is_finite() && t.max.is_finite() && t.min < t.max)
            {
                return Err(format!(
                    "{axis} travel: the minimum must be less than the maximum."
                ));
            }
        }
        if self.max_spindle_rpm.is_some_and(|v| !positive(v)) {
            return Err("Max spindle speed must be more than 0 RPM (or left blank).".into());
        }
        if self.max_feed.is_some_and(|v| !positive(v)) {
            return Err("Max feed must be more than 0 mm/min (or left blank).".into());
        }
        Ok(())
    }
}

/// The machines of this computer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Machines {
    pub machines: Vec<Machine>,
}

impl Machines {
    pub fn machine(&self, name: &str) -> Option<&Machine> {
        self.machines.iter().find(|m| m.name == name)
    }

    fn machine_mut(&mut self, name: &str) -> Result<&mut Machine, String> {
        self.machines
            .iter_mut()
            .find(|m| m.name == name)
            .ok_or_else(|| format!("There is no machine called \"{name}\"."))
    }

    /// Adds a machine called "Machine 1", "Machine 2", ... with the generic post-processor.
    /// Returns its name.
    pub fn add_machine(&mut self) -> String {
        let name = (1..)
            .map(|n| format!("Machine {n}"))
            .find(|n| self.machine(n).is_none())
            .expect("there is always a free name");
        self.machines.push(Machine::new(&name));
        name
    }

    /// Renames a machine. Returns the new name as stored (trimmed). A machine can't be called
    /// "Generic" (in any capitals): that is the project's choice for no machine.
    pub fn rename_machine(&mut self, old: &str, new: &str) -> Result<String, String> {
        let new = new.trim();
        if new.is_empty() {
            return Err("A machine needs a name.".into());
        }
        // "Generic" is the project's choice for no machine (decided 2026-11-20).
        if new.eq_ignore_ascii_case(GENERIC) {
            return Err(format!(
                "\"{new}\" can't be used: \"{GENERIC}\" already means no machine (the generic post-processor)."
            ));
        }
        if new != old && self.machine(new).is_some() {
            return Err(format!("There is already a machine called \"{new}\"."));
        }
        self.machine_mut(old)?.name = new.to_string();
        Ok(new.to_string())
    }

    /// Deletes a machine. False if there is none.
    pub fn delete_machine(&mut self, name: &str) -> bool {
        let before = self.machines.len();
        self.machines.retain(|m| m.name != name);
        self.machines.len() != before
    }

    pub fn set_post(&mut self, name: &str, post: PostProcessor) -> Result<(), String> {
        self.machine_mut(name)?.post = post;
        Ok(())
    }

    /// Sets every setting of machine `name` from `settings`, except its name. Nothing changes if
    /// a setting is invalid.
    pub fn set_settings(&mut self, name: &str, settings: &Machine) -> Result<(), String> {
        settings.check()?;
        let m = self.machine_mut(name)?;
        *m = Machine {
            name: m.name.clone(),
            ..settings.clone()
        };
        Ok(())
    }

    /// The rapid rates of the project's machine (by name). No machine chosen, or one not on this
    /// computer, gives the default rates.
    pub fn rapid_rates(&self, name: Option<&str>) -> RapidRates {
        name.and_then(|n| self.machine(n))
            .map(|m| m.rapid)
            .unwrap_or_default()
    }
}

// --- Machines file ------------------------------------------------------------------------------

/// The value of the "format" field, so other JSON files are recognised as not ours.
pub const FORMAT_NAME: &str = "WHAM_CAM machines";

/// The machines file format version written by this WHAM_CAM.
pub const FORMAT_VERSION: u32 = 1;

#[derive(Serialize)]
struct FileOut<'a> {
    format: &'a str,
    format_version: u32,
    saved_by: String,
    #[serde(flatten)]
    machines: &'a Machines,
}

/// The machines as file text (pretty-printed JSON).
pub fn to_text(machines: &Machines) -> String {
    let file = FileOut {
        format: FORMAT_NAME,
        format_version: FORMAT_VERSION,
        saved_by: format!("WHAM_CAM {}", env!("CARGO_PKG_VERSION")),
        machines,
    };
    serde_json::to_string_pretty(&file).expect("machines always convert to JSON") + "\n"
}

/// Reads machines file text. Errors are plain-English messages saying what is wrong.
pub fn from_text(text: &str) -> Result<Machines, String> {
    const NOT_OURS: &str = "This isn't a WHAM_CAM machines file.";
    let value: Value = serde_json::from_str(text).map_err(|_| NOT_OURS.to_string())?;
    if value.get("format").and_then(Value::as_str) != Some(FORMAT_NAME) {
        return Err(NOT_OURS.into());
    }
    let version = value
        .get("format_version")
        .and_then(Value::as_u64)
        .ok_or("The machines file has no format version; it may be damaged.")?;
    if version > u64::from(FORMAT_VERSION) {
        return Err(
            "The machines file was made with a newer WHAM_CAM. Please update WHAM_CAM.".into(),
        );
    }
    let machines: Machines = serde_json::from_value(value)
        .map_err(|e| format!("The machines file is damaged ({e})."))?;
    let mut names = HashSet::new();
    for m in &machines.machines {
        if m.name.trim().is_empty() {
            return Err("The machines file is damaged: a machine has no name.".into());
        }
        if !names.insert(m.name.as_str()) {
            return Err(format!(
                "The machines file is damaged: there are two machines called \"{}\".",
                m.name
            ));
        }
        m.check()
            .map_err(|e| format!("The machines file is damaged: machine \"{}\": {e}", m.name))?;
    }
    Ok(machines)
}
