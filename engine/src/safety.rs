//! The safety checker: tests a list of moves against the safety rules in spec section 7. It runs
//! on every toolpath as it is generated, and will run on the G-code too (read back into moves).

use crate::toolpath::{Limits, Move, Point3};

/// Small allowance for rounding (mm).
const EPS: f64 = 1e-6;

/// Checks `moves` against the rules. The tool is assumed to start at or above the clearance
/// plane. Returns every rule broken, in plain English (empty = safe).
///
/// 1. The first move starts at or above the clearance plane, and the last ends at it.
/// 2. No rapid moves sideways below the clearance plane.
/// 3. A rapid below the clearance plane may only go straight down (the final approach), and
///    must stop above the deepest cut so far (above the stock surface before any cut).
/// 4. No move goes below the target depth.
/// 5. Arcs never go up. They may go down as they turn (a helix, e.g. helical drilling), and rule
///    4 still stops them going below the target depth.
pub fn check(moves: &[Move], limits: &Limits) -> Vec<String> {
    let mut problems = Vec::new();
    let mut add = |i: usize, msg: String| problems.push(format!("Move {}: {msg}", i + 1));
    let clear = limits.clearance_plane;

    // Where the tool is; unknown in XY before the first move.
    let mut at: Option<Point3> = None;
    // The deepest Z cut so far (the stock surface before any cut).
    let mut deepest = limits.stock_surface;

    for (i, m) in moves.iter().enumerate() {
        let to = m.to();
        if at.is_none() && to.z < clear - EPS {
            add(i, "the first move must be at the clearance plane.".into());
        }
        if to.z < limits.target_depth - EPS {
            add(
                i,
                format!("goes below the target depth ({:.3}).", limits.target_depth),
            );
        }
        match *m {
            Move::Rapid { .. } => {
                if let Some(from) = at {
                    let sideways = (to.x - from.x).hypot(to.y - from.y) > EPS;
                    let below = to.z < clear - EPS || from.z < clear - EPS;
                    if sideways && below {
                        add(
                            i,
                            "a rapid moves sideways below the clearance plane.".into(),
                        );
                    }
                    if !sideways
                        && to.z < from.z - EPS
                        && to.z < clear - EPS
                        && to.z <= deepest + EPS
                    {
                        add(
                            i,
                            format!(
                                "a rapid goes down to Z {:.3}, not above the material (deepest cut so far {:.3}).",
                                to.z, deepest
                            ),
                        );
                    }
                }
            }
            Move::Feed { .. } => {}
            Move::Arc { .. } => {
                if let Some(from) = at
                    && to.z > from.z + EPS
                {
                    add(i, "an arc goes up.".into());
                }
            }
        }
        if !matches!(m, Move::Rapid { .. }) {
            deepest = deepest.min(to.z);
        }
        at = Some(to);
    }

    if let Some(last) = at
        && (last.z - clear).abs() > EPS
    {
        add(
            moves.len() - 1,
            "the toolpath must end at the clearance plane.".into(),
        );
    }
    problems
}
