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

/// The shape of a tool's cutting end. v1 toolpaths are for flat end mills only.
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
            EndType::Flat => "flat end mill",
            EndType::Ball => "ball nose",
            EndType::VBit => "V-bit",
            EndType::Drill => "drill",
        }
    }
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
    pub flute_length: f64,
    pub overall_length: f64,
    /// Included angle of the point (V-bit only).
    #[serde(default = "default_v_angle")]
    pub v_angle: f64,
}

impl Tool {
    /// A new tool with sensible starting values, for the Add tool form.
    pub fn new_default() -> Self {
        Self {
            id: 0,
            name: "New tool".into(),
            end_type: EndType::Flat,
            diameter: 6.0,
            shank_diameter: None,
            flutes: 2,
            flute_length: 20.0,
            overall_length: 60.0,
            v_angle: default_v_angle(),
        }
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
        if self.end_type == EndType::VBit {
            ok(
                self.v_angle > 0.0 && self.v_angle < 180.0,
                "The V angle must be more than 0° and less than 180°.",
            )?;
        }
        Ok(())
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
    pub fn add_tool(&mut self, mut tool: Tool) -> Result<u32, String> {
        tool.name = tool.name.trim().to_string();
        tool.validate()?;
        tool.id = self.tools.iter().map(|t| t.id).max().unwrap_or(0) + 1;
        let id = tool.id;
        self.tools.push(tool);
        Ok(id)
    }

    /// Replaces a master tool's values (matched by id).
    pub fn set_tool(&mut self, mut tool: Tool) -> Result<(), String> {
        tool.name = tool.name.trim().to_string();
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
