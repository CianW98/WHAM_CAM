//! Profile toolpath tests, including a move-by-move comparison with the reference example's
//! cut loop.

use wham_engine::display::polyline_signed_area;
use wham_engine::geometry::{Polyline, Vertex};
use wham_engine::operation::{
    CommonSettings, CutOrdering, MillingDirection, ProfileSettings, ProfileSide,
};
use wham_engine::profile::profile_toolpath;
use wham_engine::safety::check;
use wham_engine::toolpath::{Move, Point3, Toolpath};

const TOL: f64 = 1e-3;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < TOL
}

fn square(x0: f64, y0: f64, size: f64) -> Polyline {
    Polyline {
        vertices: vec![
            Vertex::line(x0, y0),
            Vertex::line(x0 + size, y0),
            Vertex::line(x0 + size, y0 + size),
            Vertex::line(x0, y0 + size),
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

fn side(side: ProfileSide) -> ProfileSettings {
    ProfileSettings {
        side,
        ..ProfileSettings::default()
    }
}

fn profile(paths: &[(u32, Polyline)], c: &CommonSettings, p: &ProfileSettings) -> Toolpath {
    let tp = profile_toolpath(1, paths, c, p).unwrap();
    let problems = check(&tp.moves, &tp.limits);
    assert!(problems.is_empty(), "safety checker: {problems:?}");
    tp
}

fn same_point(a: Point3, b: Point3) -> bool {
    close(a.x, b.x) && close(a.y, b.y) && close(a.z, b.z)
}

fn same_move(got: &Move, want: &Move) -> bool {
    same_point(got.to(), want.to())
        && match (got, want) {
            (Move::Rapid { .. }, Move::Rapid { .. }) => true,
            (Move::Feed { feed: a, .. }, Move::Feed { feed: b, .. }) => close(*a, *b),
            (
                Move::Arc {
                    center_x: ax,
                    center_y: ay,
                    ccw: ac,
                    feed: af,
                    ..
                },
                Move::Arc {
                    center_x: bx,
                    center_y: by,
                    ccw: bc,
                    feed: bf,
                    ..
                },
            ) => close(*ax, *bx) && close(*ay, *by) && ac == bc && close(*af, *bf),
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

fn arc(x: f64, y: f64, z: f64, cx: f64, cy: f64, ccw: bool, f: f64) -> Move {
    Move::Arc {
        to: Point3::new(x, y, z),
        center_x: cx,
        center_y: cy,
        ccw,
        feed: f,
    }
}

/// The cut moves (Feed and Arc) at height `z`.
fn cuts_at(tp: &Toolpath, z: f64) -> Vec<Move> {
    let mut at: Option<Point3> = None;
    let mut out = Vec::new();
    for m in &tp.moves {
        let sideways = at.is_some_and(|a| (a.x - m.to().x).hypot(a.y - m.to().y) > 1e-9);
        if !matches!(m, Move::Rapid { .. }) && sideways && close(m.to().z, z) {
            out.push(*m);
        }
        at = Some(m.to());
    }
    out
}

/// The reference Profile example: an 8-point polyline with arcs, 6 mm tool, Outside,
/// Conventional, target −5 in one 5 mm pass, clearance 10, feeds 1200 / 600.
/// The reference enters with a spiral lead-in (set on purpose in the reference); WHAM_CAM v1
/// plunges straight down at the start point instead (user's decision, 2026-11-16), so the entry
/// differs but the cut loop must match move for move.
#[test]
fn matches_the_reference_profile_loop_move_for_move() {
    let v = |x: f64, y: f64, bulge: f64| Vertex { x, y, bulge };
    let shape = Polyline {
        vertices: vec![
            v(-13.5991651223692, 29.0712686279469, 0.0),
            v(-10.0, 39.0, 0.0),
            v(64.0, 5.0, 0.0),
            v(31.0, -36.0, 0.0),
            v(-39.0, -41.0, 0.0),
            v(-30.9753411612932, -18.8630101001193, -0.04098469246601959),
            v(-34.747548783982, -17.0794021658196, -0.5773502691896257),
            v(-34.747548783982, 27.0794021658196, -0.218213101023905),
        ],
        closed: true,
    };
    let c = CommonSettings {
        tool_diameter: 6.0,
        target_depth: Some(-5.0),
        depth_increment: 5.0,
        clearance_plane: 10.0,
        cut_feedrate: 1200.0,
        plunge_feedrate: 600.0,
        spindle_speed: 1000.0,
        ..CommonSettings::default()
    };
    let tp = profile(&[(4, shape)], &c, &side(ProfileSide::Outside));
    let z = -5.0;
    let f = 1200.0;
    let want = vec![
        // Entry: straight plunge at the start (the reference uses a spiral lead-in here).
        rapid(-15.4621, 32.7349, 10.0),
        rapid(-15.4621, 32.7349, 1.0),
        feed(-15.4621, 32.7349, z, 600.0),
        // The loop, as in the reference G-code.
        arc(-36.2475, 29.6775, z, -22.0, 5.0, true, f),
        arc(-36.2475, -19.6775, z, -22.0, 5.0, true, f),
        arc(-34.7532, -20.4819, z, -22.0, 5.0, true, f),
        feed(-41.8204, -39.9776, z, f),
        arc(-38.7863, -43.9924, z, -39.0, -41.0, true, f),
        feed(31.2137, -38.9924, z, f),
        arc(33.337, -37.881, z, 31.0, -36.0, true, f),
        feed(66.337, 3.119, z, f),
        arc(65.2525, 7.726, z, 64.0, 5.0, true, f),
        feed(-8.7475, 41.726, z, f),
        arc(-12.8204, 40.0224, z, -10.0, 39.0, true, f),
        feed(-15.4621, 32.7349, z, f),
        rapid(-15.4621, 32.7349, 10.0),
    ];
    assert_eq!(tp.moves.len(), want.len(), "moves: {:#?}", tp.moves);
    for (i, (g, w)) in tp.moves.iter().zip(&want).enumerate() {
        assert!(same_move(g, w), "move {}: got {g:?}, want {w:?}", i + 1);
    }
}

#[test]
fn inside_has_sharp_corners_and_runs_clockwise() {
    let tp = profile(
        &[(1, square(0.0, 0.0, 40.0))],
        &settings(6.0, -2.0, 2.0),
        &side(ProfileSide::Inside),
    );
    let cuts = cuts_at(&tp, -2.0);
    assert!(
        cuts.iter().all(|m| matches!(m, Move::Feed { .. })),
        "{cuts:?}"
    );
    // Starts at the corner nearest the first point (0,0), offset 3 in: (3,3). Clockwise from
    // there goes up the left side first.
    assert!(same_point(tp.moves[0].to(), Point3::new(3.0, 3.0, 3.0)));
    assert!(same_point(cuts[0].to(), Point3::new(3.0, 37.0, -2.0)));
    assert_eq!(cuts.len(), 4);
}

#[test]
fn outside_gets_tool_radius_corner_arcs_anticlockwise() {
    let tp = profile(
        &[(1, square(0.0, 0.0, 40.0))],
        &settings(6.0, -2.0, 2.0),
        &side(ProfileSide::Outside),
    );
    let cuts = cuts_at(&tp, -2.0);
    let arcs: Vec<&Move> = cuts
        .iter()
        .filter(|m| matches!(m, Move::Arc { .. }))
        .collect();
    assert_eq!(arcs.len(), 4);
    for a in arcs {
        if let Move::Arc {
            to,
            center_x,
            center_y,
            ccw,
            ..
        } = a
        {
            assert!(*ccw);
            assert!(close((to.x - center_x).hypot(to.y - center_y), 3.0));
        }
    }
}

#[test]
fn on_line_closed_cuts_like_outside_and_climb_reverses() {
    // The square is drawn anticlockwise; draw it clockwise to show the drawn direction is ignored.
    let cw = square(0.0, 0.0, 20.0).reversed();
    for (dir, ccw) in [
        (MillingDirection::Conventional, true),
        (MillingDirection::Climb, false),
    ] {
        let p = ProfileSettings {
            side: ProfileSide::OnLine,
            milling_direction: dir,
            ..ProfileSettings::default()
        };
        let tp = profile(&[(1, cw.clone())], &settings(6.0, -1.0, 1.0), &p);
        let cuts = cuts_at(&tp, -1.0);
        let start = tp.moves[0].to();
        // Starts at the shape's first point.
        let first = cw.vertices[0];
        assert!(same_point(start, Point3::new(first.x, first.y, 3.0)));
        let mut pts = vec![Vertex::line(start.x, start.y)];
        pts.extend(cuts.iter().map(|m| Vertex::line(m.to().x, m.to().y)));
        pts.pop(); // back at the start
        let area = polyline_signed_area(&Polyline {
            vertices: pts,
            closed: true,
        });
        assert_eq!(area > 0.0, ccw, "{dir:?}");
        assert!(close(area.abs(), 400.0));
    }
}

#[test]
fn open_polyline_follows_its_drawn_direction_each_level() {
    let open = Polyline {
        vertices: vec![
            Vertex::line(0.0, 0.0),
            Vertex::line(30.0, 0.0),
            Vertex::line(30.0, 20.0),
        ],
        closed: false,
    };
    let tp = profile(
        &[(1, open)],
        &settings(6.0, -2.0, 1.0),
        &side(ProfileSide::OnLine),
    );
    for z in [-1.0, -2.0] {
        let cuts = cuts_at(&tp, z);
        assert_eq!(cuts.len(), 2);
        assert!(same_point(cuts[0].to(), Point3::new(30.0, 0.0, z)));
        assert!(same_point(cuts[1].to(), Point3::new(30.0, 20.0, z)));
    }
    // Between levels it goes back up and over to the first point.
    let rapids_to_start = tp
        .moves
        .iter()
        .filter(|m| matches!(m, Move::Rapid { to } if close(to.x, 0.0) && close(to.y, 0.0) && close(to.z, 3.0)))
        .count();
    assert_eq!(rapids_to_start, 2);
}

#[test]
fn open_shapes_must_be_on_line() {
    let open = Polyline {
        vertices: vec![Vertex::line(0.0, 0.0), Vertex::line(30.0, 0.0)],
        closed: false,
    };
    let err = profile_toolpath(
        1,
        &[(7, open)],
        &settings(6.0, -1.0, 1.0),
        &side(ProfileSide::Outside),
    )
    .unwrap_err();
    assert!(err.contains("Shape 7 is open"), "{err}");
}

#[test]
fn tool_too_big_for_inside_is_refused() {
    let err = profile_toolpath(
        1,
        &[(3, square(0.0, 0.0, 5.0))],
        &settings(6.0, -1.0, 1.0),
        &side(ProfileSide::Inside),
    )
    .unwrap_err();
    assert!(err.contains("too big") && err.contains('3'), "{err}");
}

#[test]
fn overlap_carries_on_past_the_start_on_every_level() {
    // Outside a square: start at the corner arc next to (0,0); a 5 mm overlap ends part-way
    // along the first side after the start.
    let p = ProfileSettings {
        side: ProfileSide::OnLine,
        overlap: 5.0,
        ..ProfileSettings::default()
    };
    let tp = profile(
        &[(1, square(0.0, 0.0, 20.0))],
        &settings(6.0, -2.0, 1.0),
        &p,
    );
    for z in [-1.0, -2.0] {
        let cuts = cuts_at(&tp, z);
        assert_eq!(cuts.len(), 5, "{cuts:?}");
        assert!(same_point(cuts[3].to(), Point3::new(0.0, 0.0, z)));
        assert!(same_point(cuts[4].to(), Point3::new(5.0, 0.0, z)));
    }

    // Part-way along an arc: a circle of radius 10 (two half arcs), overlap a quarter turn.
    let circle = Polyline {
        vertices: vec![
            Vertex {
                x: 10.0,
                y: 0.0,
                bulge: 1.0,
            },
            Vertex {
                x: -10.0,
                y: 0.0,
                bulge: 1.0,
            },
        ],
        closed: true,
    };
    let p = ProfileSettings {
        side: ProfileSide::OnLine,
        overlap: 10.0 * std::f64::consts::FRAC_PI_2,
        ..ProfileSettings::default()
    };
    let tp = profile(&[(1, circle)], &settings(6.0, -1.0, 1.0), &p);
    let cuts = cuts_at(&tp, -1.0);
    let last = cuts.last().unwrap();
    assert!(
        same_point(last.to(), Point3::new(0.0, 10.0, -1.0)),
        "{last:?}"
    );
    assert!(matches!(last, Move::Arc { ccw: true, .. }));
}

#[test]
fn shapes_are_cut_nearest_next() {
    // Listed: far right, far left, middle. Cut: the first listed, then the nearest (middle),
    // then the left one.
    let shapes = [
        (1, square(100.0, 0.0, 10.0)),
        (2, square(0.0, 0.0, 10.0)),
        (3, square(50.0, 0.0, 10.0)),
    ];
    let tp = profile(
        &shapes,
        &settings(3.0, -1.0, 1.0),
        &side(ProfileSide::OnLine),
    );
    // x of each plunge, in order.
    let starts: Vec<f64> = tp
        .moves
        .iter()
        .filter(|m| matches!(m, Move::Feed { feed, .. } if *feed == 300.0))
        .map(|m| m.to().x)
        .collect();
    assert_eq!(starts, [100.0, 50.0, 0.0]);
}

#[test]
fn level_first_cuts_each_level_across_all_shapes() {
    let shapes = [(1, square(0.0, 0.0, 10.0)), (2, square(50.0, 0.0, 10.0))];
    let order = |ordering: CutOrdering| -> Vec<(f64, f64)> {
        let p = ProfileSettings {
            side: ProfileSide::OnLine,
            cut_ordering: ordering,
            ..ProfileSettings::default()
        };
        let tp = profile(&shapes, &settings(3.0, -2.0, 1.0), &p);
        // (x, z) of each plunge.
        tp.moves
            .iter()
            .filter(|m| matches!(m, Move::Feed { feed, .. } if *feed == 300.0))
            .map(|m| (m.to().x, m.to().z))
            .collect()
    };
    assert_eq!(
        order(CutOrdering::DepthFirst),
        [(0.0, -1.0), (0.0, -2.0), (50.0, -1.0), (50.0, -2.0)]
    );
    assert_eq!(
        order(CutOrdering::LevelFirst),
        [(0.0, -1.0), (50.0, -1.0), (0.0, -2.0), (50.0, -2.0)]
    );
}
