//! Tool playback data (timing at the feed rates, move numbers, agreement with the drawn
//! toolpath) and the cut width display.

use std::f64::consts::PI;

use wham_engine::display::{MoveKind, playback, toolpath_display};
use wham_engine::toolpath::{Limits, Move, Point3, Toolpath};
use wham_engine::*;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn limits() -> Limits {
    Limits {
        clearance_plane: 3.0,
        stock_surface: 0.0,
        target_depth: -1.0,
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

/// Start at (0,0,3), rapid down, plunge, cut a line, a half circle, then retract.
fn sample(id: u32) -> Toolpath {
    Toolpath {
        operation_id: id,
        moves: vec![
            rapid(0.0, 0.0, 3.0),
            rapid(0.0, 0.0, 1.0),
            feed(0.0, 0.0, -1.0, 300.0),
            feed(10.0, 0.0, -1.0, 600.0),
            Move::Arc {
                to: Point3::new(20.0, 0.0, -1.0),
                center_x: 15.0,
                center_y: 0.0,
                ccw: false,
                feed: 600.0,
            },
            rapid(20.0, 0.0, 3.0),
        ],
        limits: limits(),
    }
}

#[test]
fn durations_follow_the_feed_rates() {
    let tp = sample(1);
    let pb = playback(&[(&tp, 6.0)]);
    let s = &pb.steps;
    assert_eq!(s.len(), 6);
    // The first move is where the tool starts: no time.
    assert!(close(s[0].duration_s, 0.0));
    // Rapid 2 mm down at the assumed rapid rate.
    assert!(close(s[1].duration_s, 2.0 / ASSUMED_RAPID_RATE * 60.0));
    assert_eq!(s[1].feed, None);
    assert_eq!(s[1].kind, MoveKind::Rapid);
    // Plunge 2 mm at 300 mm/min = 0.4 s.
    assert!(close(s[2].duration_s, 0.4));
    assert_eq!(s[2].kind, MoveKind::Plunge);
    // Cut 10 mm at 600 mm/min = 1 s.
    assert!(close(s[3].duration_s, 1.0));
    assert_eq!(s[3].kind, MoveKind::Cut);
    // Half circle of radius 5: true arc length 5π (not the 10 mm chord) at 600 mm/min.
    assert!(close(s[4].duration_s, 5.0 * PI / 600.0 * 60.0));
    assert_eq!(s[4].feed, Some(600.0));
    // Total is the sum, and each step starts when the last one ended.
    let sum: f64 = s.iter().map(|x| x.duration_s).sum();
    assert!(close(pb.total_s, sum));
    for w in s.windows(2) {
        assert!(close(w[1].start_s, w[0].start_s + w[0].duration_s));
    }
    assert!(s.iter().all(|x| x.tool_diameter == 6.0));
}

#[test]
fn clockwise_arc_goes_the_right_way_round() {
    let tp = sample(1);
    let pb = playback(&[(&tp, 6.0)]);
    // Clockwise from (10,0) to (20,0) around (15,0) passes over the top (y > 0).
    let arc = &pb.steps[4];
    let top = arc.points.iter().map(|p| p[1]).fold(f64::MIN, f64::max);
    assert!(top > 4.99, "highest point {top}");
}

#[test]
fn move_numbers_and_end_points_match_the_toolpath() {
    let tp = sample(1);
    let pb = playback(&[(&tp, 6.0)]);
    for (i, (step, m)) in pb.steps.iter().zip(&tp.moves).enumerate() {
        assert_eq!(step.move_number, i + 1);
        let end = step.points.last().unwrap();
        let to = m.to();
        assert_eq!(*end, [to.x, to.y, to.z]);
    }
    for w in pb.steps.windows(2) {
        assert_eq!(w[1].points[0], *w[0].points.last().unwrap());
    }
}

#[test]
fn second_operation_starts_where_the_first_ended() {
    let a = sample(1);
    let mut b = sample(2);
    for m in &mut b.moves {
        let (Move::Rapid { to } | Move::Feed { to, .. } | Move::Arc { to, .. }) = m;
        to.x += 50.0;
        if let Move::Arc { center_x, .. } = m {
            *center_x += 50.0;
        }
    }
    let pb = playback(&[(&a, 6.0), (&b, 3.0)]);
    assert_eq!(pb.steps.len(), 12);
    let first_of_b = &pb.steps[6];
    assert_eq!(first_of_b.operation_id, 2);
    assert_eq!(first_of_b.move_number, 1);
    assert_eq!(first_of_b.tool_diameter, 3.0);
    // From the end of the first (20,0,3) across to (50,0,3) at the rapid rate.
    assert_eq!(first_of_b.points[0], [20.0, 0.0, 3.0]);
    assert!(close(
        first_of_b.duration_s,
        30.0 / ASSUMED_RAPID_RATE * 60.0
    ));
}

#[test]
fn playback_points_are_the_drawn_points() {
    let tp = sample(1);
    let pb = playback(&[(&tp, 6.0)]);
    let drawn: Vec<[f64; 3]> = toolpath_display(&tp)
        .runs
        .iter()
        .flat_map(|r| r.points.clone())
        .collect();
    let mut played: Vec<[f64; 3]> = Vec::new();
    for step in &pb.steps[1..] {
        for p in &step.points {
            if played.last() != Some(p) {
                played.push(*p);
            }
        }
    }
    let mut drawn_unique: Vec<[f64; 3]> = Vec::new();
    for p in drawn {
        if drawn_unique.last() != Some(&p) {
            drawn_unique.push(p);
        }
    }
    assert_eq!(played, drawn_unique);
}

#[test]
fn project_plays_shown_toolpaths_with_their_tool() {
    let mut proj = project();
    let shape = proj
        .add_shape(
            "Default",
            Shape::rectangle(Point::new(0.0, 0.0), 50.0, 50.0, 0.0),
        )
        .unwrap();
    let op = proj.add_pocket(&[shape]).unwrap();
    let mut o = proj.operation(op).unwrap().clone();
    o.common.target_depth = Some(-2.0);
    proj.set_operation(o.clone()).unwrap();
    proj.generate(&[]);
    let pb = proj.playback();
    let tp = proj.toolpath(op).unwrap();
    assert_eq!(pb.steps.len(), tp.moves.len());
    assert!(pb.total_s > 0.0);
    assert!(pb.steps.iter().all(|s| s.tool_diameter == 6.0));

    // Hidden toolpaths are not played.
    o.show_toolpath = false;
    proj.set_operation(o).unwrap();
    assert!(proj.playback().steps.is_empty());
}

// --- Cut width (display of the band the cutter sweeps) ---

use wham_engine::display::cut_width_steps;

/// A new project with a 6 mm end mill (T1) as the Machining tool, which new operations use.
fn project() -> Project {
    let mut proj = Project::new();
    proj.set_machining_tool(Some(PickedTool {
        number: 1,
        tool: Tool {
            diameter: 6.0,
            ..Tool::new_default()
        },
    }))
    .unwrap();
    proj
}

fn in_tri(p: (f64, f64), t: &[f64]) -> bool {
    let (a, b, c) = ((t[0], t[1]), (t[3], t[4]), (t[6], t[7]));
    let s = |p: (f64, f64), q: (f64, f64), r: (f64, f64)| {
        (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0)
    };
    let (d1, d2, d3) = (s(p, a, b), s(p, b, c), s(p, c, a));
    let neg = d1 < -1e-12 || d2 < -1e-12 || d3 < -1e-12;
    let pos = d1 > 1e-12 || d2 > 1e-12 || d3 > 1e-12;
    !(neg && pos)
}

fn dist_to_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    (p.0 - a.0 - dx * t).hypot(p.1 - a.1 - dy * t)
}

#[test]
fn cut_width_covers_the_cutter_band_and_nothing_else() {
    // An L-shaped cut: (0,0) → (20,0) → (20,15), with a 6 mm cutter, after a rapid and plunge.
    let tp = Toolpath {
        operation_id: 1,
        moves: vec![
            rapid(0.0, 0.0, 3.0),
            feed(0.0, 0.0, -1.0, 300.0),
            feed(20.0, 0.0, -1.0, 600.0),
            feed(20.0, 15.0, -1.0, 600.0),
            rapid(20.0, 15.0, 3.0),
        ],
        limits: limits(),
    };
    let pb = playback(&[(&tp, 6.0)]);
    let widths = cut_width_steps(&pb);
    assert_eq!(widths.len(), pb.steps.len());
    // Each step has a strip start for each of its segments, in order.
    for (w, st) in widths.iter().zip(&pb.steps) {
        assert_eq!(w.segment_starts.len(), st.points.len() - 1);
        assert_eq!(w.triangles.len() % 9, 0);
        assert!(w.segment_starts.windows(2).all(|p| p[0] <= p[1]));
    }
    let all: Vec<f64> = widths.iter().flat_map(|w| w.triangles.clone()).collect();
    // All at the cut height.
    assert!(all.chunks(3).all(|p| close(p[2], -1.0)));
    let tris: Vec<&[f64]> = all.chunks(9).collect();
    let covered = |p: (f64, f64)| tris.iter().any(|t| in_tri(p, t));
    let dist = |p: (f64, f64)| {
        dist_to_segment(p, (0.0, 0.0), (20.0, 0.0)).min(dist_to_segment(
            p,
            (20.0, 0.0),
            (20.0, 15.0),
        ))
    };
    // A grid of points: inside the band (with a little margin for the rounded parts drawn as
    // straight steps) is covered, outside it isn't.
    let mut x = -5.0;
    while x <= 25.0 {
        let mut y = -5.0;
        while y <= 20.0 {
            let d = dist((x, y));
            if d < 2.95 {
                assert!(
                    covered((x, y)),
                    "({x}, {y}) is {d} from the path but not shaded"
                );
            } else if d > 3.01 {
                assert!(
                    !covered((x, y)),
                    "({x}, {y}) is {d} from the path but shaded"
                );
            }
            y += 0.25;
        }
        x += 0.25;
    }
}

#[test]
fn rapids_and_plunges_have_no_cut_width() {
    let tp = Toolpath {
        operation_id: 1,
        moves: vec![
            rapid(0.0, 0.0, 3.0),
            feed(0.0, 0.0, -1.0, 300.0),
            rapid(0.0, 0.0, 3.0),
        ],
        limits: limits(),
    };
    let widths = cut_width_steps(&playback(&[(&tp, 6.0)]));
    assert!(widths.iter().all(|w| w.triangles.is_empty()));
}

#[test]
fn a_helix_descends_evenly_and_takes_its_true_length() {
    // Half a turn of radius 5 going down 2 mm (spiral drilling).
    let tp = Toolpath {
        operation_id: 1,
        moves: vec![
            rapid(5.0, 0.0, 3.0),
            feed(5.0, 0.0, 0.0, 300.0),
            Move::Arc {
                to: Point3::new(-5.0, 0.0, -2.0),
                center_x: 0.0,
                center_y: 0.0,
                ccw: true,
                feed: 600.0,
            },
            rapid(-5.0, 0.0, 3.0),
        ],
        limits: limits(),
    };
    let pb = playback(&[(&tp, 6.0)]);
    let helix = &pb.steps[2];
    let zs: Vec<f64> = helix.points.iter().map(|p| p[2]).collect();
    assert!(
        zs.windows(2).all(|w| w[1] < w[0]),
        "goes down all the way: {zs:?}"
    );
    assert!(close(*zs.last().unwrap(), -2.0));
    let length = (5.0 * PI).hypot(2.0);
    assert!(close(helix.duration_s, length / 600.0 * 60.0));
}
