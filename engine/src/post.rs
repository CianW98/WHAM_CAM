//! The post-processor (spec 7): turns toolpaths into G-code text. It decides only *how* the
//! program is written (codes, number format, tool changes, start and end), never where the tool
//! goes: every move comes from the toolpath as previewed. A post-processor is a set of format
//! choices ([`PostFormat`]); v1 has only the generic one.

use std::ops::Range;

use crate::operation::SpindleDirection;
use crate::toolpath::{CycleKind, DrillCycle, Move, Point3, Toolpath};

/// The format choices of a post-processor.
#[derive(Debug, Clone, PartialEq)]
pub struct PostFormat {
    /// Decimal places for coordinates (trailing zeros trimmed).
    pub decimals: usize,
    /// Seconds to wait after starting the spindle (`G4 P…`); 0 = no dwell.
    pub spindle_dwell_s: f64,
}

impl PostFormat {
    /// The generic post-processor (decided 2026-11-20): 3 decimals, `G4 P2` after the spindle
    /// starts, `T# M6` tool changes, `I J` arcs, `G98 G81/G83` drilling.
    pub fn generic() -> Self {
        Self {
            decimals: 3,
            spindle_dwell_s: 2.0,
        }
    }
}

/// What the header says about the program.
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub project: String,
    /// When it was made, as the interface shows it (the engine has no clock).
    pub date: String,
    /// The machine's name, or None.
    pub machine: Option<String>,
    /// The post-processor's name, e.g. "Generic".
    pub post: String,
}

/// One operation of the job, with everything the post-processor writes for it.
#[derive(Debug, Clone, PartialEq)]
pub struct JobOperation<'a> {
    pub name: String,
    /// E.g. "Pocket" or "Drill [Peck]".
    pub kind: String,
    pub tool_number: u32,
    pub tool_name: String,
    pub tool_diameter: f64,
    pub spindle: SpindleDirection,
    pub spindle_speed: f64,
    pub stock_surface: f64,
    pub target_depth: f64,
    /// Shown in the comment when the operation cuts in depth passes.
    pub depth_increment: Option<f64>,
    /// Used as the controller's peck back-off when reading `G83` back (spec 6.4).
    pub fast_plunge_height: f64,
    pub toolpath: &'a Toolpath,
}

/// Which lines of the program hold one operation's toolpath moves.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub operation_id: u32,
    /// Line indexes (0-based) of the toolpath's moves, not the comments and setup before them.
    pub moves: Range<usize>,
}

/// A written program.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub lines: Vec<String>,
    pub sections: Vec<Section>,
}

impl Program {
    pub fn text(&self) -> String {
        let mut t = self.lines.join("\n");
        t.push('\n');
        t
    }
}

/// A number with up to `decimals` decimals, trailing zeros trimmed: 12.5, 3, -0.25.
pub fn number(v: f64, decimals: usize) -> String {
    let s = format!("{v:.decimals$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if s == "-0" { "0".into() } else { s }
}

/// A comment's text with nothing that could end it early.
pub fn comment(text: &str) -> String {
    let clean: String = text
        .chars()
        .map(|c| match c {
            '(' => '[',
            ')' => ']',
            '\n' | '\r' => ' ',
            c => c,
        })
        .collect();
    format!("( {} )", clean.trim())
}

/// Writes lines, remembering the machine's modal state so only words that change are written.
struct Writer<'f> {
    f: &'f PostFormat,
    lines: Vec<String>,
    /// Where the tool is, as written (rounded text of each axis; None = unknown).
    x: Option<String>,
    y: Option<String>,
    z: Option<String>,
    feed: Option<String>,
    /// Z as a number (for the lift between operations).
    z_value: Option<f64>,
}

impl Writer<'_> {
    fn n(&self, v: f64) -> String {
        number(v, self.f.decimals)
    }

    fn line(&mut self, s: impl Into<String>) {
        self.lines.push(s.into());
    }

    /// The axis words for moving to `to`: only the ones that change (all when unknown).
    fn axes(&mut self, to: Point3) -> String {
        let mut words = Vec::new();
        for (letter, v) in [('X', to.x), ('Y', to.y), ('Z', to.z)] {
            let text = self.n(v);
            let slot = match letter {
                'X' => &mut self.x,
                'Y' => &mut self.y,
                _ => &mut self.z,
            };
            if slot.as_deref() != Some(text.as_str()) {
                words.push(format!("{letter}{text}"));
                *slot = Some(text);
            }
        }
        self.z_value = Some(to.z);
        words.join(" ")
    }

    fn feed_word(&mut self, feed: f64) -> Option<String> {
        let text = number(feed.round(), 0);
        if self.feed.as_deref() == Some(text.as_str()) {
            None
        } else {
            self.feed = Some(text.clone());
            Some(format!("F{text}"))
        }
    }

    fn write_move(&mut self, m: &Move) {
        match *m {
            Move::Rapid { to } => {
                let axes = self.axes(to);
                if !axes.is_empty() {
                    self.line(format!("G0 {axes}"));
                }
            }
            Move::Feed { to, feed } => {
                let axes = self.axes(to);
                let mut l = format!("G1 {axes}");
                if let Some(f) = self.feed_word(feed) {
                    l = format!("{} {f}", l.trim_end());
                }
                if !axes.is_empty() {
                    self.line(l);
                }
            }
            Move::Arc {
                to,
                center_x,
                center_y,
                ccw,
                feed,
            } => {
                // I J from the start as written (rounded), so the controller's centre is as
                // close to the true centre as the rounding allows.
                let sx: f64 = self.x.as_deref().map_or(0.0, |v| v.parse().unwrap_or(0.0));
                let sy: f64 = self.y.as_deref().map_or(0.0, |v| v.parse().unwrap_or(0.0));
                let i = self.n(center_x - sx);
                let j = self.n(center_y - sy);
                // Arcs always write X and Y (a full circle ends where it starts).
                let x = self.n(to.x);
                let y = self.n(to.y);
                self.x = Some(x.clone());
                self.y = Some(y.clone());
                let mut l = format!("{} X{x} Y{y}", if ccw { "G3" } else { "G2" });
                let z = self.n(to.z);
                if self.z.as_deref() != Some(z.as_str()) {
                    l.push_str(&format!(" Z{z}"));
                    self.z = Some(z);
                }
                self.z_value = Some(to.z);
                l.push_str(&format!(" I{i} J{j}"));
                if let Some(f) = self.feed_word(feed) {
                    l.push_str(&format!(" {f}"));
                }
                self.line(l);
            }
        }
    }

    /// One drilling cycle line: `G98 G81/G83 X Y Z R (Q) F` for the first hole of a block, then
    /// only the words that change (e.g. `Y8`), as the cycle stays in force until `G80`.
    fn write_cycle(&mut self, c: &DrillCycle, first: bool, last: Option<&DrillCycle>) {
        let code = match c.kind {
            CycleKind::Standard => "G81",
            CycleKind::Peck => "G83",
        };
        // The cycle is modal: only its first line names it; later holes are just their
        // coordinates (and anything else that changes) until G80.
        let mut words = if first {
            vec![format!("G98 {code}")]
        } else {
            Vec::new()
        };
        let x = self.n(c.x);
        let y = self.n(c.y);
        if first || self.x.as_deref() != Some(x.as_str()) {
            words.push(format!("X{x}"));
        }
        if first || self.y.as_deref() != Some(y.as_str()) {
            words.push(format!("Y{y}"));
        }
        self.x = Some(x);
        self.y = Some(y);
        let same = |f: fn(&DrillCycle) -> f64| last.is_some_and(|l| self.n(f(l)) == self.n(f(c)));
        if first || !same(|c| c.z) {
            words.push(format!("Z{}", self.n(c.z)));
        }
        if first || !same(|c| c.r) {
            words.push(format!("R{}", self.n(c.r)));
        }
        if c.kind == CycleKind::Peck && (first || !same(|c| c.q)) {
            words.push(format!("Q{}", self.n(c.q)));
        }
        if let Some(f) = self.feed_word(c.feed) {
            words.push(f);
        }
        self.line(words.join(" "));
    }
}

/// The spindle state as written.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Spindle {
    Off,
    On(SpindleDirection, f64),
}

/// Writes the whole job (operations in order) as one program.
pub fn write_program(header: &Header, job: &[JobOperation], f: &PostFormat) -> Program {
    let mut w = Writer {
        f,
        lines: Vec::new(),
        x: None,
        y: None,
        z: None,
        feed: None,
        z_value: None,
    };
    // Header: version, project, post, machine, the tools used.
    w.line(comment(&format!("WHAM_CAM {}", env!("CARGO_PKG_VERSION"))));
    w.line(comment(&format!(
        "Project: {}   {}",
        header.project, header.date
    )));
    w.line(comment(&format!(
        "Post-processor: {}   Machine: {}",
        header.post,
        header.machine.as_deref().unwrap_or("none")
    )));
    let mut listed: Vec<u32> = Vec::new();
    for op in job {
        if !listed.contains(&op.tool_number) {
            listed.push(op.tool_number);
            w.line(comment(&format!(
                "T{}: {}, D{}",
                op.tool_number,
                op.tool_name,
                w.n(op.tool_diameter)
            )));
        }
    }
    w.line("G21 G90 G17");
    if let Some(first) = job.first() {
        let clear = first.toolpath.limits.clearance_plane;
        w.line(format!("G0 Z{}", w.n(clear)));
        w.z = Some(w.n(clear));
        w.z_value = Some(clear);
    }

    let mut tool: Option<u32> = None;
    let mut spindle = Spindle::Off;
    let mut sections = Vec::new();
    for op in job {
        let tp = op.toolpath;
        let clear = tp.limits.clearance_plane;
        let mut about = format!(
            "{}: {}, T{} D{}, stock {}, target {}",
            op.name,
            op.kind,
            op.tool_number,
            w.n(op.tool_diameter),
            w.n(op.stock_surface),
            w.n(op.target_depth)
        );
        if let Some(inc) = op.depth_increment {
            about.push_str(&format!(", depth increment {}", w.n(inc)));
        }
        w.line(comment(&about));
        // Up to this operation's clearance plane first if it is higher (straight up).
        if w.z_value.is_some_and(|z| z < clear - 1e-9) {
            w.line(format!("G0 Z{}", w.n(clear)));
            w.z = Some(w.n(clear));
            w.z_value = Some(clear);
        }
        if tool != Some(op.tool_number) {
            if spindle != Spindle::Off {
                w.line("M5");
                spindle = Spindle::Off;
            }
            w.line(comment(&format!("T{}: {}", op.tool_number, op.tool_name)));
            w.line(format!("T{} M6", op.tool_number));
            tool = Some(op.tool_number);
        }
        let want = match op.spindle {
            SpindleDirection::Off => Spindle::Off,
            d => Spindle::On(d, op.spindle_speed.round()),
        };
        if want != spindle {
            match want {
                Spindle::Off => w.line("M5"),
                Spindle::On(d, speed) => {
                    let m = if d == SpindleDirection::CCW {
                        "M4"
                    } else {
                        "M3"
                    };
                    w.line(format!("S{} {m}", number(speed, 0)));
                    if f.spindle_dwell_s > 0.0 {
                        w.line(format!("G4 P{}", number(f.spindle_dwell_s, 3)));
                    }
                }
            }
            spindle = want;
        }
        // Drill cycles start from this operation's clearance plane exactly (G98 returns there).
        if !tp.cycles.is_empty() && w.z.as_deref() != Some(w.n(clear).as_str()) {
            w.line(format!("G0 Z{}", w.n(clear)));
            w.z = Some(w.n(clear));
            w.z_value = Some(clear);
        }
        let start = w.lines.len();
        if tp.cycles.is_empty() {
            for m in &tp.moves {
                w.write_move(m);
            }
        } else {
            let mut last: Option<&DrillCycle> = None;
            for c in &tp.cycles {
                w.write_cycle(c, last.is_none_or(|l| l.kind != c.kind), last);
                last = Some(c);
            }
            w.line("G80");
            // After G98 the tool is back at the clearance plane over the last hole.
            w.z = Some(w.n(clear));
            w.z_value = Some(clear);
        }
        sections.push(Section {
            operation_id: tp.operation_id,
            moves: start..w.lines.len(),
        });
    }
    if spindle != Spindle::Off {
        w.line("M5");
    }
    w.line("M30");
    Program {
        lines: w.lines,
        sections,
    }
}
