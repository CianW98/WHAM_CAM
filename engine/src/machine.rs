//! Machines (spec 5.7): the machines G-code is written for, with their settings. Like the
//! tooling, the machine list belongs to the computer: a project keeps a copy of its machine's
//! settings, which follow the machine except the ones changed in the project. The engine turns the list into text and back; the app reads and writes
//! the file.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The built-in machine: always in the list, can't be renamed, edited or deleted.
pub const DEFAULT_MACHINE: &str = "Default";

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

/// Rapid rate of the Default machine (mm/min).
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

/// A machine's settings. A project keeps its own copy (spec 5.7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MachineSettings {
    pub post: PostProcessor,
    pub rapid: RapidRates,
    /// Has an automatic tool changer. Stored only for now.
    pub tool_changer: bool,
    /// Travel limits per axis; None = not set. Stored only for now.
    pub travel_x: Option<Travel>,
    pub travel_y: Option<Travel>,
    pub travel_z: Option<Travel>,
    pub spindle_control: SpindleControl,
    /// None = not set. Stored only for now.
    pub max_spindle_rpm: Option<f64>,
    /// Highest cutting feed rate (mm/min); None = not set. Stored only for now.
    pub max_feed: Option<f64>,
}

impl Default for MachineSettings {
    /// The Default machine's settings.
    fn default() -> Self {
        MachineSettings {
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
}

/// One machine setting, to say which ones a project has changed for itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MachineSetting {
    Post,
    RapidXy,
    RapidZ,
    ToolChanger,
    TravelX,
    TravelY,
    TravelZ,
    SpindleControl,
    MaxSpindleRpm,
    MaxFeed,
}

impl MachineSetting {
    pub const ALL: [MachineSetting; 10] = [
        MachineSetting::Post,
        MachineSetting::RapidXy,
        MachineSetting::RapidZ,
        MachineSetting::ToolChanger,
        MachineSetting::TravelX,
        MachineSetting::TravelY,
        MachineSetting::TravelZ,
        MachineSetting::SpindleControl,
        MachineSetting::MaxSpindleRpm,
        MachineSetting::MaxFeed,
    ];
}

impl MachineSettings {
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

    /// True if `setting` has a different value in `other`.
    pub fn differs(&self, other: &MachineSettings, setting: MachineSetting) -> bool {
        use MachineSetting as S;
        match setting {
            S::Post => self.post != other.post,
            S::RapidXy => self.rapid.xy != other.rapid.xy,
            S::RapidZ => self.rapid.z != other.rapid.z,
            S::ToolChanger => self.tool_changer != other.tool_changer,
            S::TravelX => self.travel_x != other.travel_x,
            S::TravelY => self.travel_y != other.travel_y,
            S::TravelZ => self.travel_z != other.travel_z,
            S::SpindleControl => self.spindle_control != other.spindle_control,
            S::MaxSpindleRpm => self.max_spindle_rpm != other.max_spindle_rpm,
            S::MaxFeed => self.max_feed != other.max_feed,
        }
    }

    /// Copies one setting from `from`.
    pub fn copy(&mut self, from: &MachineSettings, setting: MachineSetting) {
        use MachineSetting as S;
        match setting {
            S::Post => self.post = from.post,
            S::RapidXy => self.rapid.xy = from.rapid.xy,
            S::RapidZ => self.rapid.z = from.rapid.z,
            S::ToolChanger => self.tool_changer = from.tool_changer,
            S::TravelX => self.travel_x = from.travel_x,
            S::TravelY => self.travel_y = from.travel_y,
            S::TravelZ => self.travel_z = from.travel_z,
            S::SpindleControl => self.spindle_control = from.spindle_control,
            S::MaxSpindleRpm => self.max_spindle_rpm = from.max_spindle_rpm,
            S::MaxFeed => self.max_feed = from.max_feed,
        }
    }
}

/// One machine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Machine {
    /// Unique among the machines.
    pub name: String,
    #[serde(flatten)]
    pub settings: MachineSettings,
}

impl Machine {
    /// The built-in Default machine.
    pub fn built_in() -> Machine {
        Machine {
            name: DEFAULT_MACHINE.to_string(),
            settings: MachineSettings::default(),
        }
    }
}

/// The machines of this computer. The first is always the built-in Default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Machines {
    pub machines: Vec<Machine>,
    /// The machine File ▸ New (and startup) uses. Always one of `machines`.
    #[serde(default = "default_machine_name")]
    pub for_new_projects: String,
}

fn default_machine_name() -> String {
    DEFAULT_MACHINE.to_string()
}

impl Default for Machines {
    fn default() -> Self {
        Machines {
            machines: vec![Machine::built_in()],
            for_new_projects: default_machine_name(),
        }
    }
}

/// The error for changing the Default machine.
fn read_only() -> String {
    format!(
        "\"{DEFAULT_MACHINE}\" is built in and can't be changed. Use New from to make an editable copy."
    )
}

impl Machines {
    pub fn machine(&self, name: &str) -> Option<&Machine> {
        self.machines.iter().find(|m| m.name == name)
    }

    /// A machine other than Default, to change.
    fn editable(&mut self, name: &str) -> Result<&mut Machine, String> {
        if name == DEFAULT_MACHINE {
            return Err(read_only());
        }
        self.machines
            .iter_mut()
            .find(|m| m.name == name)
            .ok_or_else(|| format!("There is no machine called \"{name}\"."))
    }

    /// The machine new projects start with.
    pub fn for_new(&self) -> &Machine {
        self.machine(&self.for_new_projects)
            .unwrap_or(&self.machines[0])
    }

    /// The first free name of the form "<base> 1", "<base> 2", ... (or `first` if free).
    fn free_name(&self, first: Option<&str>, base: &str) -> String {
        first
            .filter(|n| self.machine(n).is_none())
            .map(str::to_string)
            .unwrap_or_else(|| {
                (1..)
                    .map(|n| format!("{base} {n}"))
                    .find(|n| self.machine(n).is_none())
                    .expect("there is always a free name")
            })
    }

    /// Adds a machine called "Machine 1", "Machine 2", ... with the Default machine's settings.
    /// Returns its name.
    pub fn add_machine(&mut self) -> String {
        let name = self.free_name(None, "Machine");
        self.machines.push(Machine {
            name: name.clone(),
            settings: MachineSettings::default(),
        });
        name
    }

    /// Adds an editable copy of machine `from`, called "<from> copy" (then "<from> copy 2", ...).
    /// Returns its name.
    pub fn new_from(&mut self, from: &str) -> Result<String, String> {
        let settings = self
            .machine(from)
            .ok_or_else(|| format!("There is no machine called \"{from}\"."))?
            .settings
            .clone();
        let base = format!("{from} copy");
        let name = self.free_name(Some(&base), &base);
        self.machines.push(Machine {
            name: name.clone(),
            settings,
        });
        Ok(name)
    }

    /// Renames a machine. Returns the new name as stored (trimmed). Default can't be renamed, and
    /// no other machine can be called "Default" (in any capitals).
    pub fn rename_machine(&mut self, old: &str, new: &str) -> Result<String, String> {
        let new = new.trim();
        if new.is_empty() {
            return Err("A machine needs a name.".into());
        }
        if new.eq_ignore_ascii_case(DEFAULT_MACHINE) && old != DEFAULT_MACHINE {
            return Err(format!(
                "\"{new}\" can't be used: it is the built-in machine's name."
            ));
        }
        if new != old && self.machine(new).is_some() {
            return Err(format!("There is already a machine called \"{new}\"."));
        }
        self.editable(old)?.name = new.to_string();
        if self.for_new_projects == old {
            self.for_new_projects = new.to_string();
        }
        Ok(new.to_string())
    }

    /// Deletes a machine. Default can't be deleted. If it was used for new projects, Default is
    /// used instead.
    pub fn delete_machine(&mut self, name: &str) -> Result<(), String> {
        self.editable(name)?;
        self.machines.retain(|m| m.name != name);
        if self.for_new_projects == name {
            self.for_new_projects = default_machine_name();
        }
        Ok(())
    }

    /// Sets every setting of machine `name`. Nothing changes if a setting is invalid.
    pub fn set_settings(&mut self, name: &str, settings: &MachineSettings) -> Result<(), String> {
        settings.check()?;
        self.editable(name)?.settings = settings.clone();
        Ok(())
    }

    /// Makes File ▸ New start with this machine.
    pub fn set_for_new_projects(&mut self, name: &str) -> Result<(), String> {
        if self.machine(name).is_none() {
            return Err(format!("There is no machine called \"{name}\"."));
        }
        self.for_new_projects = name.to_string();
        Ok(())
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
    let mut machines: Machines = serde_json::from_value(value)
        .map_err(|e| format!("The machines file is damaged ({e})."))?;
    // Default is built in: whatever the file says about it, it has the built-in settings.
    machines
        .machines
        .retain(|m| !m.name.eq_ignore_ascii_case(DEFAULT_MACHINE));
    machines.machines.insert(0, Machine::built_in());
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
        m.settings
            .check()
            .map_err(|e| format!("The machines file is damaged: machine \"{}\": {e}", m.name))?;
    }
    if machines.machine(&machines.for_new_projects).is_none() {
        return Err(format!(
            "The machines file is damaged: the machine for new projects, \"{}\", isn't in the list.",
            machines.for_new_projects
        ));
    }
    Ok(machines)
}
