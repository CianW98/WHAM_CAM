//! Drill toolpaths (spec 6.4): Standard (straight down), Peck, or Spiral (a helix round a hole
//! bigger than the tool), at every hole position, nearest hole next. Standard and Peck holes are
//! canned cycles (G81 / G83), whose moves come from [`cycle_moves`].

use std::f64::consts::PI;

use crate::geometry::{Point, Shape, on_circle};
use crate::operation::{CommonSettings, DrillMethod, DrillSettings, MillingDirection};
use crate::toolpath::{CycleKind, DrillCycle, Limits, Move, Point3, Toolpath};

/// One hole to drill.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hole {
    pub at: Point,
    /// The hole's own diameter, for a circle: spiral drilling cuts it to this size. `None` (point
    /// lists, arcs) uses the operation's Hole diameter setting.
    pub diameter: Option<f64>,
}

/// The holes a shape gives: every point of a point list, the centre of a circle (with its
/// diameter) or of an arc.
pub fn drill_points(id: u32, shape: &Shape) -> Result<Vec<Hole>, String> {
    let at = |p: Point| Hole {
        at: p,
        diameter: None,
    };
    match shape {
        Shape::Points { points } if !points.is_empty() => {
            Ok(points.iter().map(|p| at(*p)).collect())
        }
        Shape::Points { .. } => Err(format!("Shape {id} is an empty point list.")),
        Shape::Circle { center, radius } => Ok(vec![Hole {
            at: *center,
            diameter: Some(2.0 * radius),
        }]),
        Shape::Arc { center, .. } => Ok(vec![at(*center)]),
        _ => Err(format!(
            "Shape {id} is a {}: drilling uses point lists, circles and arcs (their centres).",
            shape.type_name()
        )),
    }
}

/// Moves for one operation, tracking where the tool is.
struct Moves {
    moves: Vec<Move>,
    at: Option<Point3>,
}

impl Moves {
    fn push(&mut self, m: Move) {
        self.at = Some(m.to());
        self.moves.push(m);
    }
    fn rapid(&mut self, x: f64, y: f64, z: f64) {
        self.push(Move::Rapid {
            to: Point3::new(x, y, z),
        });
    }
    fn feed(&mut self, x: f64, y: f64, z: f64, feed: f64) {
        self.push(Move::Feed {
            to: Point3::new(x, y, z),
            feed,
        });
    }
}

/// A drill toolpath. `holes` are the selected shapes' hole positions with the shape ids (for
/// messages), in the order listed. Errors are plain-English messages.
pub fn drill_toolpath(
    operation_id: u32,
    holes: &[(u32, Vec<Hole>)],
    c: &CommonSettings,
    d: &DrillSettings,
) -> Result<Toolpath, String> {
    let target = c.target_depth.ok_or("Set the target depth.")?;
    // Each hole with the shape it came from (for messages).
    let mut left: Vec<(u32, Hole)> = holes
        .iter()
        .flat_map(|(id, hs)| hs.iter().map(move |h| (*id, *h)))
        .collect();
    if left.is_empty() {
        return Err(
            "The drill has no holes. Select point lists, circles or arcs and use Use selected shapes."
                .into(),
        );
    }
    // The first listed hole, then always the nearest hole not yet drilled.
    if d.method == DrillMethod::Spiral {
        for (id, h) in &left {
            let dia = h.diameter.unwrap_or(d.hole_diameter);
            if dia <= c.tool_diameter {
                return Err(match h.diameter {
                    Some(_) => format!(
                        "Circle {id} ({dia} mm) is not bigger than the tool, so it can't be spiral drilled."
                    ),
                    None => "For spiral drilling the hole diameter must be bigger than the tool diameter."
                        .into(),
                });
            }
        }
    }
    let mut order: Vec<Hole> = Vec::with_capacity(left.len());
    let first = left.remove(0).1;
    let mut here = first.at;
    order.push(first);
    while !left.is_empty() {
        let mut best = (0, f64::INFINITY);
        for (i, (_, p)) in left.iter().enumerate() {
            let dist = p.at.distance_to(here);
            if dist < best.1 - 1e-9 {
                best = (i, dist);
            }
        }
        let next = left.remove(best.0).1;
        here = next.at;
        order.push(next);
    }

    let clear = c.clearance_plane;
    let mut m = Moves {
        moves: Vec::new(),
        at: None,
    };
    let mut cycles = Vec::new();
    for hole in order {
        let h = hole.at;
        let kind = match d.method {
            DrillMethod::Standard => Some(CycleKind::Standard),
            DrillMethod::Peck => Some(CycleKind::Peck),
            DrillMethod::Spiral => None,
        };
        match kind {
            Some(kind) => {
                // Standard and Peck are canned cycles: the moves come from the cycle.
                let cycle = DrillCycle {
                    kind,
                    x: h.x,
                    y: h.y,
                    r: d.retract_height,
                    z: target,
                    q: if kind == CycleKind::Peck {
                        d.peck_distance
                    } else {
                        0.0
                    },
                    feed: c.plunge_feedrate,
                };
                for mv in cycle_moves(&cycle, clear, c.fast_plunge_height) {
                    m.push(mv);
                }
                cycles.push(cycle);
            }
            None => {
                let dia = hole.diameter.unwrap_or(d.hole_diameter);
                spiral(&mut m, h, dia, target, c, d);
                // Straight up out of the hole, from wherever the tool is (it may end at the wall).
                let end = m.at.map_or(h, |a| Point::new(a.x, a.y));
                m.rapid(end.x, end.y, clear);
            }
        }
    }

    Ok(Toolpath {
        operation_id,
        moves: m.moves,
        limits: Limits {
            clearance_plane: clear,
            stock_surface: c.stock_surface,
            target_depth: target,
        },
        cycles,
    })
}

/// The moves of one canned drilling cycle, as a controller runs `G98 G81` / `G98 G83` with the
/// tool starting at the clearance plane (the "initial plane" G98 returns to). The one place these
/// moves are made: the toolpath, its preview and the G-code read-back all use it.
///
/// - Rapid over the hole at the clearance plane, then rapid down to the R plane (if lower).
/// - G81: feed to the bottom.
/// - G83: feed down `q` at a time, counted from the R plane (so pecks above the stock cut air, as
///   on the machine). After each peck: rapid back up to R to clear chips, then rapid back down to
///   `back_off` above the last depth and feed on. The controller sets that back-off itself; the
///   preview uses the fast plunge height for it.
/// - Rapid back up to the clearance plane.
pub fn cycle_moves(cycle: &DrillCycle, clearance: f64, back_off: f64) -> Vec<Move> {
    let (x, y) = (cycle.x, cycle.y);
    let r = cycle.r;
    let mut m = Moves {
        moves: Vec::new(),
        at: None,
    };
    m.rapid(x, y, clearance);
    if r < clearance {
        m.rapid(x, y, r);
    }
    match cycle.kind {
        CycleKind::Standard => m.feed(x, y, cycle.z, cycle.feed),
        CycleKind::Peck => {
            let mut depth = r;
            loop {
                let next = (depth - cycle.q).max(cycle.z);
                if depth < r {
                    // Back into the hole: rapid to just above the last depth, then feed.
                    let fast = depth + back_off;
                    if fast < r {
                        m.rapid(x, y, fast);
                    }
                }
                m.feed(x, y, next, cycle.feed);
                depth = next;
                if depth <= cycle.z + 1e-9 {
                    break;
                }
                m.rapid(x, y, r); // out of the hole to clear chips
            }
        }
    }
    m.rapid(x, y, clearance);
    m.moves
}

/// A spiral hole at `h`: from the wall position, feed down to the stock surface, helix down by
/// at most the depth increment per turn to the target, then one flat circle at the bottom.
/// If the tool has cleared the centre (hole no more than twice the tool diameter) it then feeds
/// to the centre, off the wall, ready to go up. A bigger hole leaves an uncut core in the middle,
/// so the tool stays at the wall and goes straight up from there (moving inward would run into
/// the core).
fn spiral(
    m: &mut Moves,
    h: Point,
    hole_diameter: f64,
    target: f64,
    c: &CommonSettings,
    d: &DrillSettings,
) {
    let path_r = (hole_diameter - c.tool_diameter) / 2.0;
    let ccw = d.milling_direction == MillingDirection::Climb;
    let start = Point::new(h.x + path_r, h.y);
    m.rapid(start.x, start.y, c.clearance_plane);
    if d.retract_height < c.clearance_plane {
        m.rapid(start.x, start.y, d.retract_height);
    }
    m.feed(start.x, start.y, c.stock_surface, c.plunge_feedrate);
    // Equal turns, none deeper than the depth increment.
    let total = c.stock_surface - target;
    let turns = ((total / c.depth_increment) - 1e-9).ceil().max(1.0) as usize;
    let per_half = total / (2 * turns) as f64;
    let half = |m: &mut Moves, k: usize, z: f64| {
        // Half turns end alternately on the far side and back at the start.
        let p = if k.is_multiple_of(2) {
            on_circle(h, path_r, PI)
        } else {
            start
        };
        m.push(Move::Arc {
            to: Point3::new(p.x, p.y, z),
            center_x: h.x,
            center_y: h.y,
            ccw,
            feed: c.cut_feedrate,
        });
    };
    for k in 0..2 * turns {
        let z = if k == 2 * turns - 1 {
            target
        } else {
            c.stock_surface - per_half * (k + 1) as f64
        };
        half(m, k, z);
    }
    // Flat bottom: one full circle at the target depth.
    half(m, 0, target);
    half(m, 1, target);
    let centre_cleared = path_r <= c.tool_diameter / 2.0 + 1e-9;
    if centre_cleared {
        m.feed(h.x, h.y, target, c.cut_feedrate);
    }
}
