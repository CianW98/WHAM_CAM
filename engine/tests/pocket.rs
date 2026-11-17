//! Pocket toolpath tests, including a move-by-move comparison with the reference example.

use wham_engine::geometry::{Polyline, Vertex};
use wham_engine::operation::{CommonSettings, CutOrdering, MillingDirection, PocketSettings};
use wham_engine::pocket::{closed_outline, pocket_toolpath};
use wham_engine::safety::check;
use wham_engine::toolpath::{Move, Point3, Toolpath, depth_levels};
use wham_engine::*;

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Polyline {
    Polyline {
        vertices: vec![
            Vertex::line(x0, y0),
            Vertex::line(x1, y0),
            Vertex::line(x1, y1),
            Vertex::line(x0, y1),
        ],
        closed: true,
    }
}

fn settings(diameter: f64, target: f64, increment: f64) -> CommonSettings {
    CommonSettings {
        tool_diameter: diameter,
        target_depth: Some(target),
        depth_increment: increment,
        ..CommonSettings::default()
    }
}

fn pocket(outlines: &[(u32, Polyline)], c: &CommonSettings, p: &PocketSettings) -> Toolpath {
    let tp = pocket_toolpath(1, outlines, c, p).unwrap();
    let problems = check(&tp.moves, &tp.limits);
    assert!(problems.is_empty(), "safety checker: {problems:?}");
    tp
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn same_move(got: &Move, want: &Move) -> bool {
    let (g, w) = (got.to(), want.to());
    let at = close(g.x, w.x) && close(g.y, w.y) && close(g.z, w.z);
    at && match (got, want) {
        (Move::Rapid { .. }, Move::Rapid { .. }) => true,
        (Move::Feed { feed: a, .. }, Move::Feed { feed: b, .. }) => close(*a, *b),
        _ => false,
    }
}

fn rapid(x: f64, y: f64, z: f64) -> Move {
    Move::Rapid {
        to: Point3::new(x, y, z),
    }
}

fn feed(x: f64, y: f64, z: f64, f: f64) -> Move {
    Move::Feed {
        to: Point3::new(x, y, z),
        feed: f,
    }
}

/// The reference Pocket example: a 100 × 80 rectangle, 3 mm tool, stepover 0.4, target −10 in
/// 5 mm passes, clearance 3, feeds 800 / 300 (plunge, also used for stepovers).
#[test]
fn matches_the_reference_pocket_move_for_move() {
    let c = CommonSettings {
        cut_feedrate: 800.0,
        plunge_feedrate: 300.0,
        clearance_plane: 3.0,
        ..settings(3.0, -10.0, 5.0)
    };
    let tp = pocket(
        &[(1, rect(0.0, 0.0, 100.0, 80.0))],
        &c,
        &PocketSettings::default(),
    );

    // Expected moves, written out from the reference G-code.
    let mut want = Vec::new();
    for (level, z) in [(0, -5.0), (1, -10.0)] {
        if level == 0 {
            want.push(rapid(39.9, 39.9, 3.0));
            want.push(rapid(39.9, 39.9, 1.0));
        } else {
            want.push(rapid(39.9, 1.5, 3.0));
            want.push(rapid(39.9, 39.9, 3.0));
            want.push(rapid(39.9, 39.9, -4.0));
        }
        want.push(feed(39.9, 39.9, z, 300.0));
        // 33 rings, from 39.9 out to 1.5 in steps of 1.2, cut clockwise.
        for k in (0..=32).rev() {
            let d = 1.5 + 1.2 * k as f64;
            if k < 32 {
                want.push(feed(39.9, d, z, 300.0)); // stepover from the last ring
                want.push(feed(d, d, z, 800.0));
            }
            want.push(feed(d, 80.0 - d, z, 800.0));
            want.push(feed(100.0 - d, 80.0 - d, z, 800.0));
            want.push(feed(100.0 - d, d, z, 800.0));
            want.push(feed(39.9, d, z, 800.0));
        }
    }
    want.push(rapid(39.9, 1.5, 3.0));

    for (i, (g, w)) in tp.moves.iter().zip(&want).enumerate() {
        assert!(same_move(g, w), "move {}: got {g:?}, want {w:?}", i + 1);
    }
    assert_eq!(tp.moves.len(), want.len());
}

#[test]
fn depth_levels_are_equal_passes_with_optional_final_pass() {
    assert_eq!(depth_levels(0.0, -10.0, 5.0, 0.0), vec![-5.0, -10.0]);
    let l = depth_levels(0.0, -10.0, 3.0, 0.0);
    assert_eq!(l.len(), 4);
    assert!(close(l[0], -2.5) && close(l[3], -10.0));
    let l = depth_levels(0.0, -10.0, 3.0, 0.5);
    assert!(
        close(*l.last().unwrap(), -10.0) && close(l[l.len() - 2], -9.5),
        "{l:?}"
    );
    assert_eq!(depth_levels(1.0, -1.0, 5.0, 0.0), vec![-1.0]);
}

/// Every XY position the tool cuts at (feed moves) is at least a tool radius from `p`'s outline.
fn min_distance_to_rect(tp: &Toolpath, x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    tp.moves
        .iter()
        .filter(|m| !matches!(m, Move::Rapid { .. }))
        .map(|m| {
            let t = m.to();
            let dx = (x0 - t.x).max(t.x - x1).max(0.0);
            let dy = (y0 - t.y).max(t.y - y1).max(0.0);
            dx.hypot(dy)
        })
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn island_is_left_uncut_with_rounded_corners() {
    let outlines = [
        (1, rect(0.0, 0.0, 100.0, 80.0)),
        (2, rect(40.0, 30.0, 60.0, 50.0)),
    ];
    let tp = pocket(
        &outlines,
        &settings(6.0, -2.0, 2.0),
        &PocketSettings::default(),
    );
    // The tool centre never comes closer than its radius to the island.
    let gap = min_distance_to_rect(&tp, 40.0, 30.0, 60.0, 50.0);
    assert!(gap >= 3.0 - 1e-6, "tool centre {gap} mm from the island");
    // Rounded corners round the island: anticlockwise arcs (conventional, outside wall).
    let arcs = tp
        .moves
        .iter()
        .filter(|m| matches!(m, Move::Arc { ccw: true, .. }))
        .count();
    assert!(arcs >= 4, "{arcs} arcs");
}

#[test]
fn shape_inside_an_island_is_pocketed_on_its_own() {
    let outlines = [
        (1, rect(0.0, 0.0, 100.0, 100.0)),
        (2, rect(20.0, 20.0, 80.0, 80.0)),
        (3, rect(40.0, 40.0, 60.0, 60.0)),
    ];
    let tp = pocket(
        &outlines,
        &settings(4.0, -1.0, 1.0),
        &PocketSettings::default(),
    );
    let inside_small = tp.moves.iter().any(|m| {
        let t = m.to();
        t.z < 0.0 && t.x > 40.0 && t.x < 60.0 && t.y > 40.0 && t.y < 60.0
    });
    assert!(inside_small, "the innermost square is pocketed");
    let in_ring = tp.moves.iter().any(|m| {
        let t = m.to();
        t.z < 0.0 && t.x > 22.0 && t.x < 38.0 && t.y > 22.0 && t.y < 38.0
    });
    assert!(!in_ring, "the island ring (between 20 and 40) is not cut");
}

#[test]
fn bad_selections_are_refused() {
    let c = settings(6.0, -1.0, 1.0);
    let p = PocketSettings::default();
    // Too big a tool.
    let err = pocket_toolpath(1, &[(7, rect(0.0, 0.0, 5.0, 5.0))], &c, &p).unwrap_err();
    assert!(err.contains("too big") && err.contains('7'), "{err}");
    // Crossing outlines.
    let crossing = [
        (1, rect(0.0, 0.0, 50.0, 50.0)),
        (2, rect(25.0, 25.0, 75.0, 75.0)),
    ];
    let err = pocket_toolpath(1, &crossing, &c, &p).unwrap_err();
    assert!(err.contains("cross"), "{err}");
    // No target depth.
    let no_depth = CommonSettings::default();
    assert!(pocket_toolpath(1, &[(1, rect(0.0, 0.0, 50.0, 50.0))], &no_depth, &p).is_err());
    // Open polyline.
    let open = Shape::Polyline(Polyline {
        closed: false,
        ..rect(0.0, 0.0, 10.0, 10.0)
    });
    assert!(closed_outline(3, &open).unwrap_err().contains("open"));
}

#[test]
fn big_stepover_adds_a_middle_ring() {
    // 16 mm square, 6 mm tool, stepover 0.9 (5.4 mm): the normal rings stop at 3 mm in (8.4 mm
    // is past the middle), which would leave the centre 5 mm from any cut. The extra middle ring
    // must bring it within a tool radius.
    let c = settings(6.0, -1.0, 1.0);
    let p = PocketSettings {
        stepover: 0.9,
        ..PocketSettings::default()
    };
    let tp = pocket(&[(1, rect(0.0, 0.0, 16.0, 16.0))], &c, &p);
    // Distance from the pocket centre to the nearest point of any cut (straight feed segments).
    let mut centre_gap = f64::INFINITY;
    for w in tp.moves.windows(2) {
        let (a, b) = (w[0].to(), w[1].to());
        if a.z >= 0.0 || b.z >= 0.0 {
            continue;
        }
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len2 = dx * dx + dy * dy;
        let t = if len2 == 0.0 {
            0.0
        } else {
            (((8.0 - a.x) * dx + (8.0 - a.y) * dy) / len2).clamp(0.0, 1.0)
        };
        centre_gap = centre_gap.min((a.x + dx * t - 8.0).hypot(a.y + dy * t - 8.0));
    }
    assert!(
        centre_gap <= 3.0 + 1e-6,
        "centre is {centre_gap} mm from the nearest cut"
    );
}

#[test]
fn climb_reverses_the_rings_and_level_first_orders_levels() {
    let outlines = [
        (1, rect(0.0, 0.0, 30.0, 30.0)),
        (2, rect(50.0, 0.0, 80.0, 30.0)),
    ];
    let c = settings(6.0, -2.0, 1.0);
    let conv = pocket(&outlines, &c, &PocketSettings::default());
    let climb = pocket(
        &outlines,
        &c,
        &PocketSettings {
            milling_direction: MillingDirection::Climb,
            ..PocketSettings::default()
        },
    );
    // Signed area of the first ring's path (positive = anticlockwise).
    let first_ring_turn = |tp: &Toolpath| {
        let pts: Vec<Point3> = tp
            .moves
            .iter()
            .skip_while(|m| matches!(m, Move::Rapid { .. }))
            .skip(1)
            .take(4)
            .map(Move::to)
            .collect();
        let mut a = 0.0;
        for i in 0..pts.len() {
            let (p, q) = (pts[i], pts[(i + 1) % pts.len()]);
            a += p.x * q.y - q.x * p.y;
        }
        a
    };
    assert!(
        first_ring_turn(&conv) < 0.0,
        "conventional: inside walls clockwise"
    );
    assert!(first_ring_turn(&climb) > 0.0, "climb: anticlockwise");

    // Depth first: all of the first pocket (x < 30) at both levels before the second.
    let zs_by_x = |tp: &Toolpath| -> Vec<(bool, f64)> {
        tp.moves
            .iter()
            .filter(|m| matches!(m, Move::Feed { .. }) && m.to().z < 0.0)
            .map(|m| (m.to().x < 40.0, m.to().z))
            .collect()
    };
    let depth = zs_by_x(&conv);
    let switch = depth.iter().position(|(left, _)| !left).unwrap();
    assert!(depth[..switch].iter().any(|(_, z)| close(*z, -2.0)));
    let level = pocket(
        &outlines,
        &c,
        &PocketSettings {
            cut_ordering: CutOrdering::LevelFirst,
            ..PocketSettings::default()
        },
    );
    let lv = zs_by_x(&level);
    let switch = lv.iter().position(|(left, _)| !left).unwrap();
    assert!(
        lv[..switch].iter().all(|(_, z)| close(*z, -1.0)),
        "level first: only level 1 before switching"
    );
}

#[test]
fn circle_pocket_uses_arcs() {
    let circle = closed_outline(
        1,
        &Shape::Circle {
            center: Point::new(0.0, 0.0),
            radius: 20.0,
        },
    )
    .unwrap();
    let tp = pocket(
        &[(1, circle)],
        &settings(6.0, -1.0, 1.0),
        &PocketSettings::default(),
    );
    assert!(tp.moves.iter().any(|m| matches!(m, Move::Arc { .. })));
    // Outermost ring is the 17 mm radius circle.
    let max_r = tp
        .moves
        .iter()
        .map(|m| m.to().x.hypot(m.to().y))
        .fold(0.0, f64::max);
    assert!(close(max_r, 17.0), "{max_r}");
}
