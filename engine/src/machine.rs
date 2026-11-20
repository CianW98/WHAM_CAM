//! Machines (spec 5.7): the machines G-code is written for, with the post-processor each uses.
//! Like the tooling, the machine list belongs to the computer, not to a project: a project only
//! names its machine. The engine turns the list into text and back; the app reads and writes
//! the file.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

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

/// One machine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Machine {
    /// Unique among the machines.
    pub name: String,
    #[serde(default)]
    pub post: PostProcessor,
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
        self.machines.push(Machine {
            name: name.clone(),
            post: PostProcessor::Generic,
        });
        name
    }

    /// Renames a machine. Returns the new name as stored (trimmed).
    pub fn rename_machine(&mut self, old: &str, new: &str) -> Result<String, String> {
        let new = new.trim();
        if new.is_empty() {
            return Err("A machine needs a name.".into());
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
    }
    Ok(machines)
}
