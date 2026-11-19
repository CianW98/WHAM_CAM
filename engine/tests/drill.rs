//! Drill toolpath tests, including a move-by-move comparison with the reference example.

use wham_engine::drill::{Hole, drill_points, drill_toolpath};
use wham_engine::geometry::{Point, Shape};
use wham_engine::operation::{CommonSettings, DrillMethod, DrillSettings, MillingDirection};
use wham_engine::safety::check;
use wham_engine::toolpath::{Move, Point3, Toolpath};

const TOL: f64 = 1e-9;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < TOL
}

fn settings(diameter: f64, target: f64) -> CommonSettings {
    CommonSettings {
        tool_diameter: diameter,
        target_depth: Some(target),
        ..CommonSettings::default()
    }
}

fn method(method: DrillMethod) -> DrillSettings {
    DrillSettings {
        method,
        ..DrillSettings::default()
    }
}

fn drill(holes: &[Point], c: &CommonSettings, d: &DrillSettings) -> Toolpath {
    let holes: Vec<Hole> = holes
        .iter()
        .map(|p| Hole {
            at: *p,
            diameter: None,
        })
        .collect();
    let tp = drill_toolpath(1, &[(1, holes)], c, d).unwrap();
    let problems = check(&tp.moves, &tp.limits);
    assert!(problems.is_empty(), "safety checker: {problems:?}");
    tp
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

fn same(got: &Move, want: &Move) -> bool {
    let (g, w) = (got.to(), want.to());
    close(g.x, w.x)
        && close(g.y, w.y)
        && close(g.z, w.z)
        && match (got, want) {
            (Move::Rapid { .. }, Move::Rapid { .. }) => true,
            (Move::Feed { feed: a, .. }, Move::Feed { feed: b, .. }) => close(*a, *b),
            _ => false,
        }
}

fn assert_moves(got: &[Move], want: &[Move]) {
    assert_eq!(got.len(), want.len(), "moves: {got:#?}");
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert!(same(g, w), "move {}: got {g:?}, want {w:?}", i + 1);
    }
}

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// The reference Drill example: a point list of 4 points, 1 mm tool, target −12, retract height
/// and clearance 3, plunge feed 300. The reference writes a canned cycle (G98 G81 … R3); written
/// out in full it is: rapid over the hole at Z3, feed to −12, rapid back to Z3, in the reference's
/// hole order (nearest next).
#[test]
fn matches_the_reference_drill_move_for_move() {
    let holes = [p(34.0, 22.0), p(53.0, 22.0), p(53.0, 8.0), p(34.0, 8.0)];
    let tp = drill(
        &holes,
        &settings(1.0, -12.0),
        &method(DrillMethod::Standard),
    );
    let mut want = Vec::new();
    for (x, y) in [(34.0, 22.0), (34.0, 8.0), (53.0, 8.0), (53.0, 22.0)] {
        want.push(rapid(x, y, 3.0));
        want.push(feed(x, y, -12.0, 300.0));
        want.push(rapid(x, y, 3.0));
    }
    assert_moves(&tp.moves, &want);
}

#[test]
fn standard_rapids_down_to_a_lower_retract_height_first() {
    let c = CommonSettings {
        clearance_plane: 10.0,
        ..settings(3.0, -5.0)
    };
    let d = DrillSettings {
        retract_height: 2.0,
        ..method(DrillMethod::Standard)
    };
    let tp = drill(&[p(0.0, 0.0)], &c, &d);
    assert_moves(
        &tp.moves,
        &[
            rapid(0.0, 0.0, 10.0),
            rapid(0.0, 0.0, 2.0),
            feed(0.0, 0.0, -5.0, 300.0),
            rapid(0.0, 0.0, 10.0),
        ],
    );
}

#[test]
fn peck_goes_back_to_the_retract_height_between_pecks() {
    let d = DrillSettings {
        peck_distance: 2.0,
        ..method(DrillMethod::Peck)
    };
    // Target −5 in pecks of 2: −2, −4, −5. Fast plunge height 1 above the last depth.
    let tp = drill(&[p(5.0, 5.0)], &settings(3.0, -5.0), &d);
    assert_moves(
        &tp.moves,
        &[
            rapid(5.0, 5.0, 3.0),
            feed(5.0, 5.0, -2.0, 300.0),
            rapid(5.0, 5.0, 3.0),
            rapid(5.0, 5.0, -1.0),
            feed(5.0, 5.0, -4.0, 300.0),
            rapid(5.0, 5.0, 3.0),
            rapid(5.0, 5.0, -3.0),
            feed(5.0, 5.0, -5.0, 300.0),
            rapid(5.0, 5.0, 3.0),
        ],
    );
}

#[test]
fn spiral_helixes_down_cleans_the_floor_and_leaves_from_the_centre() {
    // 10 mm hole, 6 mm tool: the tool centre goes round at radius 2. Target −5, 2 mm per turn:
    // 3 equal turns of 5/3 mm.
    let c = CommonSettings {
        depth_increment: 2.0,
        ..settings(6.0, -5.0)
    };
    let d = method(DrillMethod::Spiral);
    let tp = drill(&[p(20.0, 10.0)], &c, &d);
    let m = &tp.moves;
    // Over the wall position, then feed down to the stock surface.
    assert!(same(&m[0], &rapid(22.0, 10.0, 3.0)));
    assert!(same(&m[1], &feed(22.0, 10.0, 0.0, 300.0)));
    let arcs: Vec<&Move> = m.iter().filter(|x| matches!(x, Move::Arc { .. })).collect();
    // 3 turns of 2 half arcs, then a flat circle of 2 half arcs.
    assert_eq!(arcs.len(), 8);
    let mut z = 0.0;
    for (i, a) in arcs.iter().enumerate() {
        let Move::Arc {
            to,
            center_x,
            center_y,
            ccw,
            feed,
        } = a
        else {
            unreachable!()
        };
        assert!(close(*center_x, 20.0) && close(*center_y, 10.0));
        assert!((to.x - 20.0).hypot(to.y - 10.0) - 2.0 < 1e-9);
        assert!(!ccw, "conventional cuts the hole wall clockwise");
        assert!(close(*feed, 800.0));
        if i < 6 {
            // Down 5/6 mm each half turn: never more than the depth increment per full turn.
            assert!(
                (z - to.z - 5.0 / 6.0).abs() < 1e-9,
                "arc {i}: {z} → {}",
                to.z
            );
        } else {
            assert!(close(to.z, -5.0), "flat bottom");
        }
        z = to.z;
    }
    // The hole (10) is no more than twice the tool (6), so the centre is cleared: off the wall
    // to the centre, and straight up.
    let n = m.len();
    assert!(same(&m[n - 2], &feed(20.0, 10.0, -5.0, 800.0)));
    assert!(same(&m[n - 1], &rapid(20.0, 10.0, 3.0)));

    // Climb goes round anticlockwise.
    let d = DrillSettings {
        milling_direction: MillingDirection::Climb,
        ..d
    };
    let tp = drill(&[p(20.0, 10.0)], &c, &d);
    assert!(
        tp.moves
            .iter()
            .all(|x| !matches!(x, Move::Arc { ccw: false, .. }))
    );
}

#[test]
fn holes_are_drilled_nearest_next() {
    let holes = [p(100.0, 0.0), p(0.0, 0.0), p(60.0, 0.0), p(10.0, 0.0)];
    let tp = drill(&holes, &settings(3.0, -1.0), &method(DrillMethod::Standard));
    let xs: Vec<f64> = tp
        .moves
        .iter()
        .filter(|m| matches!(m, Move::Feed { .. }))
        .map(|m| m.to().x)
        .collect();
    assert_eq!(xs, [100.0, 60.0, 10.0, 0.0]);
}

#[test]
fn hole_positions_come_from_point_lists_circles_and_arcs() {
    let pts = Shape::Points {
        points: vec![p(1.0, 2.0), p(3.0, 4.0)],
    };
    let at = |h: Vec<Hole>| h.iter().map(|h| (h.at, h.diameter)).collect::<Vec<_>>();
    assert_eq!(
        at(drill_points(1, &pts).unwrap()),
        vec![(p(1.0, 2.0), None), (p(3.0, 4.0), None)]
    );
    let circle = Shape::Circle {
        center: p(5.0, 6.0),
        radius: 2.0,
    };
    // A circle gives its centre and its own diameter.
    assert_eq!(
        at(drill_points(2, &circle).unwrap()),
        vec![(p(5.0, 6.0), Some(4.0))]
    );
    let arc = Shape::Arc {
        center: p(7.0, 8.0),
        radius: 2.0,
        start_angle: 0.0,
        end_angle: 90.0,
    };
    assert_eq!(
        at(drill_points(3, &arc).unwrap()),
        vec![(p(7.0, 8.0), None)]
    );
    let line = Shape::Line {
        start: p(0.0, 0.0),
        end: p(1.0, 0.0),
    };
    assert!(drill_points(4, &line).unwrap_err().contains("Shape 4"));
}

#[test]
fn spiral_cuts_a_circle_to_its_own_diameter() {
    // A 16 mm circle with a 6 mm tool: the tool centre goes round at radius 5, whatever the Hole
    // diameter setting says (it is for point lists and arcs).
    let circle = Shape::Circle {
        center: p(0.0, 0.0),
        radius: 8.0,
    };
    let holes = drill_points(1, &circle).unwrap();
    let c = settings(6.0, -2.0);
    let d = DrillSettings {
        hole_diameter: 10.0,
        ..method(DrillMethod::Spiral)
    };
    let tp = drill_toolpath(1, &[(1, holes)], &c, &d).unwrap();
    assert!(check(&tp.moves, &tp.limits).is_empty());
    for m in &tp.moves {
        if let Move::Arc { to, .. } = m {
            assert!((to.x.hypot(to.y) - 5.0).abs() < 1e-9, "{to:?}");
        }
    }
    // The hole is more than twice the tool, so a core is left in the middle: the tool must not
    // move towards the centre at the bottom. It goes straight up from the wall instead.
    let n = tp.moves.len();
    assert!(
        matches!(tp.moves[n - 2], Move::Arc { .. }),
        "{:?}",
        tp.moves[n - 2]
    );
    assert!(same(&tp.moves[n - 1], &rapid(5.0, 0.0, 3.0)));
    assert!(
        tp.moves
            .iter()
            .all(|m| !matches!(m, Move::Feed { to, .. } if to.x.hypot(to.y) < 5.0 - 1e-9))
    );

    // A circle no bigger than the tool can't be spiral drilled.
    let small = Shape::Circle {
        center: p(0.0, 0.0),
        radius: 3.0,
    };
    let err = drill_toolpath(1, &[(7, drill_points(7, &small).unwrap())], &c, &d).unwrap_err();
    assert!(err.contains("Circle 7"), "{err}");
}
