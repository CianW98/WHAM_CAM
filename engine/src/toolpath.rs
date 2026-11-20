//! Toolpaths: the machine moves an operation makes, before any G-code is written. The safety
//! checker (`safety.rs`) checks these, and later the post-processor turns them into G-code.

use serde::{Deserialize, Serialize};

/// A point in machine space (mm).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point3 {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

/// One machine move, from wherever the previous move ended.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Move {
    /// Rapid (G0).
    Rapid { to: Point3 },
    /// Straight feed move (G1) at `feed` mm/min.
    Feed { to: Point3, feed: f64 },
    /// Arc in the XY plane at constant Z (G2 clockwise / G3 anticlockwise) around `center`.
    Arc {
        to: Point3,
        center_x: f64,
        center_y: f64,
        ccw: bool,
        feed: f64,
    },
}

impl Move {
    pub fn to(&self) -> Point3 {
        match *self {
            Move::Rapid { to } | Move::Feed { to, .. } | Move::Arc { to, .. } => to,
        }
    }
}

/// The limits a toolpath must stay within, for the safety checker.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Limits {
    pub clearance_plane: f64,
    pub stock_surface: f64,
    pub target_depth: f64,
}

/// Which canned drilling cycle a hole is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CycleKind {
    /// G81: feed straight to the bottom.
    Standard,
    /// G83: peck drilling, `q` deep at a time counted from the R plane.
    Peck,
}

/// One hole drilled with a canned cycle (G98 G81 / G83), as the post-processor writes it. The
/// toolpath's moves for the hole are made from it by [`crate::drill::cycle_moves`], so the
/// preview and the G-code always agree.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DrillCycle {
    pub kind: CycleKind,
    pub x: f64,
    pub y: f64,
    /// R plane: the tool rapids down to it, and (peck) back up to it between pecks.
    pub r: f64,
    /// Bottom of the hole.
    pub z: f64,
    /// Peck depth (Peck only; 0 for Standard).
    pub q: f64,
    /// Feed rate into the hole, mm/min.
    pub feed: f64,
}

/// All the moves of one operation. It starts at the clearance plane (where the previous
/// operation or the program start left the tool) and ends there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Toolpath {
    pub operation_id: u32,
    pub moves: Vec<Move>,
    pub limits: Limits,
    /// For Standard and Peck drilling: each hole as a canned cycle, in order. Together they make
    /// exactly `moves` (empty for every other operation, which is written move by move).
    #[serde(default)]
    pub cycles: Vec<DrillCycle>,
}

/// The Z of each depth pass, from the first (shallowest) down to `target`. Passes are equal and
/// no deeper than `increment`. With `final_increment` > 0 (and less than the total depth) the
/// last pass is that size and the rest are equal.
pub fn depth_levels(stock: f64, target: f64, increment: f64, final_increment: f64) -> Vec<f64> {
    let total = stock - target;
    if !(total > 0.0 && increment > 0.0) {
        return Vec::new();
    }
    let passes = |depth: f64| ((depth / increment) - 1e-9).ceil().max(1.0) as usize;
    let mut levels = Vec::new();
    let rest = if final_increment > 0.0 && final_increment < total {
        total - final_increment
    } else {
        total
    };
    let n = passes(rest);
    for i in 1..=n {
        levels.push(stock - rest * i as f64 / n as f64);
    }
    if rest < total {
        levels.push(target);
    }
    // The last level is exactly the target, with no rounding.
    if let Some(last) = levels.last_mut() {
        *last = target;
    }
    levels
}
