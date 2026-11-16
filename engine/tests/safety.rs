//! Safety checker tests: each rule broken on purpose must be caught.

use wham_engine::safety::check;
use wham_engine::toolpath::{Limits, Move, Point3};

const LIMITS: Limits = Limits {
    clearance_plane: 3.0,
    stock_surface: 0.0,
    target_depth: -5.0,
};

fn rapid(x: f64, y: f64, z: f64) -> Move {
    Move::Rapid {
        to: Point3::new(x, y, z),
    }
}

fn feed(x: f64, y: f64, z: f64) -> Move {
    Move::Feed {
        to: Point3::new(x, y, z),
        feed: 300.0,
    }
}

/// A safe little toolpath: plunge, cut a line, step down, cut back, retract.
fn safe() -> Vec<Move> {
    vec![
        rapid(0.0, 0.0, 3.0),
        rapid(0.0, 0.0, 1.0),
        feed(0.0, 0.0, -2.0),
        feed(10.0, 0.0, -2.0),
        rapid(10.0, 0.0, 3.0),
        rapid(0.0, 0.0, 3.0),
        rapid(0.0, 0.0, -1.0),
        feed(0.0, 0.0, -5.0),
        Move::Arc {
            to: Point3::new(10.0, 0.0, -5.0),
            center_x: 5.0,
            center_y: 0.0,
            ccw: true,
            feed: 800.0,
        },
        rapid(10.0, 0.0, 3.0),
    ]
}

fn broken(i: usize, m: Move) -> Vec<String> {
    let mut moves = safe();
    moves[i] = m;
    check(&moves, &LIMITS)
}

#[test]
fn a_safe_toolpath_passes() {
    assert!(
        check(&safe(), &LIMITS).is_empty(),
        "{:?}",
        check(&safe(), &LIMITS)
    );
}

#[test]
fn rule_1_starts_and_ends_at_clearance() {
    let p = broken(0, rapid(0.0, 0.0, 1.0));
    assert!(p.iter().any(|m| m.contains("first move")), "{p:?}");
    let mut moves = safe();
    moves.pop();
    let p = check(&moves, &LIMITS);
    assert!(
        p.iter().any(|m| m.contains("end at the clearance")),
        "{p:?}"
    );
}

#[test]
fn rule_2_no_sideways_rapid_below_clearance() {
    // Rapid across at Z 1 instead of retracting.
    let p = broken(5, rapid(0.0, 0.0, 1.0));
    assert!(p.iter().any(|m| m.contains("sideways")), "{p:?}");
}

#[test]
fn rule_3_rapid_down_stops_above_the_material() {
    // Before any cut: a rapid down into the stock.
    let p = broken(1, rapid(0.0, 0.0, -1.0));
    assert!(
        p.iter().any(|m| m.contains("not above the material")),
        "{p:?}"
    );
    // After cutting to -2: a rapid down to -2 (not above it).
    let p = broken(6, rapid(0.0, 0.0, -2.0));
    assert!(
        p.iter().any(|m| m.contains("not above the material")),
        "{p:?}"
    );
}

#[test]
fn rule_4_never_below_target_depth() {
    let p = broken(7, feed(0.0, 0.0, -5.5));
    assert!(
        p.iter().any(|m| m.contains("below the target depth")),
        "{p:?}"
    );
}

#[test]
fn rule_5_arcs_stay_flat() {
    let p = broken(
        8,
        Move::Arc {
            to: Point3::new(10.0, 0.0, -4.0),
            center_x: 5.0,
            center_y: 0.0,
            ccw: true,
            feed: 800.0,
        },
    );
    assert!(p.iter().any(|m| m.contains("arc changes height")), "{p:?}");
}
