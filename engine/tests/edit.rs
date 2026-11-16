//! Edit tool tests: join, explode, close polyline, break at intersections.

use wham_engine::edit::{break_at_intersections, close_polyline, explode, join};
use wham_engine::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn same(a: Point, b: Point) -> bool {
    a.distance_to(b) < 1e-9
}

fn line(x0: f64, y0: f64, x1: f64, y1: f64) -> Shape {
    Shape::Line {
        start: p(x0, y0),
        end: p(x1, y1),
    }
}

fn arc(cx: f64, cy: f64, r: f64, start: f64, end: f64) -> Shape {
    Shape::Arc {
        center: p(cx, cy),
        radius: r,
        start_angle: start,
        end_angle: end,
    }
}

fn v(x: f64, y: f64, bulge: f64) -> Vertex {
    Vertex { x, y, bulge }
}

/// Both end points of a line or arc.
fn ends(s: &Shape) -> (Point, Point) {
    match s {
        Shape::Line { start, end } => (*start, *end),
        Shape::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let at = |a: f64| {
                p(
                    center.x + radius * a.to_radians().cos(),
                    center.y + radius * a.to_radians().sin(),
                )
            };
            (at(*start_angle), at(*end_angle))
        }
        Shape::Polyline(pl) => (
            pl.vertices[0].point(),
            pl.vertices[pl.vertices.len() - 1].point(),
        ),
        other => panic!("no ends: {other:?}"),
    }
}

fn polyline(s: &Shape) -> &Polyline {
    match s {
        Shape::Polyline(pl) => pl,
        other => panic!("not a polyline: {other:?}"),
    }
}

/// A polyline's corners in 0.001 mm units, sorted (for comparing outlines in any order).
fn corner_set(pl: &Polyline) -> Vec<(i64, i64)> {
    let mut c: Vec<(i64, i64)> = pl
        .vertices
        .iter()
        .map(|v| ((v.x * 1000.0).round() as i64, (v.y * 1000.0).round() as i64))
        .collect();
    c.sort();
    c
}

fn project_with(shapes: Vec<Shape>) -> (Project, Vec<u32>) {
    let mut proj = Project::new();
    let ids = shapes
        .into_iter()
        .map(|s| proj.add_shape("0", s).unwrap())
        .collect();
    (proj, ids)
}

fn shapes_of(proj: &Project) -> Vec<Shape> {
    proj.document().objects().map(|o| o.shape.clone()).collect()
}

// --- Join ---------------------------------------------------------------------------------------

#[test]
fn join_square_from_mixed_lines() {
    // Out of order and some pointing the wrong way.
    let shapes = vec![
        line(10.0, 10.0, 10.0, 0.0),
        line(0.0, 0.0, 10.0, 0.0),
        line(0.0, 10.0, 0.0, 0.0),
        line(0.0, 10.0, 10.0, 10.0),
    ];
    let joined = join(&shapes, 0.01);
    assert_eq!(joined.len(), 1);
    let pl = &joined[0].polyline;
    assert!(pl.closed);
    assert_eq!(pl.vertices.len(), 4);
    assert_eq!(
        corner_set(pl),
        vec![(0, 0), (0, 10000), (10000, 0), (10000, 10000)]
    );
    assert_eq!(joined[0].sources.len(), 4);
}

#[test]
fn join_line_arc_line_keeps_the_arc() {
    // A U shape: line up, half circle over the top (anticlockwise from 0° to 180°), line down.
    let shapes = vec![
        line(10.0, 0.0, 10.0, 10.0),
        arc(5.0, 10.0, 5.0, 0.0, 180.0),
        line(0.0, 10.0, 0.0, 0.0),
    ];
    let joined = join(&shapes, 0.01);
    assert_eq!(joined.len(), 1);
    let pl = &joined[0].polyline;
    assert!(!pl.closed);
    assert_eq!(pl.vertices.len(), 4);
    let arcs: Vec<f64> = pl.vertices.iter().map(|v| v.bulge).collect();
    assert!(
        close(arcs[0], 0.0) && close(arcs[1], 1.0) && close(arcs[2], 0.0),
        "{arcs:?}"
    );

    // The same with the arc walked the other way (chain starts from the left line).
    let shapes = vec![
        line(0.0, 0.0, 0.0, 10.0),
        arc(5.0, 10.0, 5.0, 0.0, 180.0),
        line(10.0, 10.0, 10.0, 0.0),
    ];
    let pl = &join(&shapes, 0.01)[0].polyline;
    assert!(close(pl.vertices[1].bulge, -1.0), "{:?}", pl.vertices);
    // Exploding it gives back an anticlockwise arc from 0° to 180°.
    let parts = explode(&Shape::Polyline(pl.clone())).unwrap();
    assert_eq!(parts[1], arc(5.0, 10.0, 5.0, 0.0, 180.0));
}

#[test]
fn join_respects_the_tolerance() {
    let shapes = vec![line(0.0, 0.0, 10.0, 0.0), line(10.005, 0.0, 20.0, 0.0)];
    assert_eq!(join(&shapes, 0.01).len(), 1);
    assert!(join(&shapes, 0.001).is_empty());
}

#[test]
fn join_branch_makes_several_polylines() {
    // Three lines meeting at one point: two chain together, the third stays alone.
    let shapes = vec![
        line(0.0, 0.0, 10.0, 0.0),
        line(10.0, 0.0, 20.0, 0.0),
        line(10.0, 0.0, 10.0, 10.0),
    ];
    let joined = join(&shapes, 0.01);
    assert_eq!(joined.len(), 1);
    assert_eq!(joined[0].sources.len(), 2);
}

#[test]
fn join_in_project_replaces_shapes_and_reports() {
    let (mut proj, ids) = project_with(vec![
        line(0.0, 0.0, 10.0, 0.0),
        line(10.0, 0.0, 10.0, 10.0),
        line(10.0, 10.0, 0.0, 0.0),
        Shape::Circle {
            center: p(50.0, 50.0),
            radius: 5.0,
        },
        line(100.0, 0.0, 110.0, 0.0),
    ]);
    let report = proj.join(&ids, 0.01).unwrap();
    assert_eq!(report.joined, 3);
    assert_eq!(report.closed, 1);
    assert_eq!(report.skipped, 2);
    assert_eq!(report.ids.len(), 1);
    let shapes = shapes_of(&proj);
    assert_eq!(shapes.len(), 3);
    assert!(polyline(&shapes[0]).closed);
    // One undo step puts the three lines back.
    assert!(proj.undo());
    assert_eq!(shapes_of(&proj).len(), 5);
    assert!(proj.join(&ids, 0.0).is_err());
}

// --- Explode and close --------------------------------------------------------------------------

#[test]
fn explode_polyline_with_arcs_ends_meet() {
    let pl = Polyline {
        vertices: vec![
            v(0.0, 0.0, 0.0),
            v(10.0, 0.0, 0.5),
            v(10.0, 10.0, -0.3),
            v(0.0, 10.0, 0.0),
        ],
        closed: true,
    };
    let parts = explode(&Shape::Polyline(pl)).unwrap();
    assert_eq!(parts.len(), 4);
    assert!(matches!(parts[1], Shape::Arc { .. }));
    assert!(matches!(parts[2], Shape::Arc { .. }));
    // Every piece's end meets some other piece's start or end.
    for (i, a) in parts.iter().enumerate() {
        let (s, e) = ends(a);
        for pt in [s, e] {
            let meets = parts.iter().enumerate().any(|(j, b)| {
                let (bs, be) = ends(b);
                i != j && (same(pt, bs) || same(pt, be))
            });
            assert!(meets, "piece {i} end {pt:?} meets nothing");
        }
    }
}

#[test]
fn explode_then_join_gives_back_the_outline() {
    let pl = Polyline {
        vertices: vec![
            v(0.0, 0.0, 0.0),
            v(20.0, 0.0, 0.4),
            v(20.0, 10.0, 0.0),
            v(0.0, 10.0, -0.2),
        ],
        closed: true,
    };
    let parts = explode(&Shape::Polyline(pl.clone())).unwrap();
    let joined = join(&parts, 1e-6);
    assert_eq!(joined.len(), 1);
    let back = &joined[0].polyline;
    assert!(back.closed);
    assert_eq!(back.vertices.len(), 4);
    for (a, b) in back.vertices.iter().zip(&pl.vertices) {
        assert!(
            same(a.point(), b.point()) && (a.bulge - b.bulge).abs() < 1e-9,
            "{a:?} vs {b:?}"
        );
    }
}

#[test]
fn explode_rectangle_gives_four_lines() {
    let rect = Shape::rectangle(p(0.0, 0.0), 20.0, 10.0, 30.0);
    let parts = explode(&rect).unwrap();
    assert_eq!(parts.len(), 4);
    assert!(parts.iter().all(|s| matches!(s, Shape::Line { .. })));
    assert!(explode(&line(0.0, 0.0, 1.0, 1.0)).is_none());

    let (mut proj, ids) = project_with(vec![rect, line(0.0, 0.0, 1.0, 1.0)]);
    let new = proj.explode(&ids);
    assert_eq!(new.len(), 4);
    assert_eq!(shapes_of(&proj).len(), 5);
    assert!(proj.undo());
    assert_eq!(shapes_of(&proj).len(), 2);
}

#[test]
fn close_drops_a_duplicate_end() {
    let open = Polyline {
        vertices: vec![
            v(0.0, 0.0, 0.0),
            v(10.0, 0.0, 0.0),
            v(10.0, 10.0, 0.0),
            v(0.0005, 0.0, 0.0),
        ],
        closed: false,
    };
    let closed = close_polyline(&open).unwrap();
    assert!(closed.closed);
    assert_eq!(closed.vertices.len(), 3);
    assert!(close_polyline(&closed).is_none());
    // Too short to close.
    let two = Polyline {
        vertices: vec![v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0)],
        closed: false,
    };
    assert!(close_polyline(&two).is_none());

    let (mut proj, ids) = project_with(vec![Shape::Polyline(open)]);
    assert_eq!(proj.close_polylines(&ids), 1);
    assert!(polyline(&shapes_of(&proj)[0]).closed);
    assert!(proj.undo());
    assert!(!polyline(&shapes_of(&proj)[0]).closed);
}

// --- Break at intersections ---------------------------------------------------------------------

#[test]
fn break_two_crossing_lines() {
    let shapes = vec![line(0.0, 0.0, 10.0, 10.0), line(0.0, 10.0, 10.0, 0.0)];
    let (broken, crossings) = break_at_intersections(&shapes);
    assert_eq!(crossings, 1);
    assert_eq!(broken.len(), 2);
    for b in &broken {
        assert_eq!(b.pieces.len(), 2);
        let (s, e) = ends(&b.pieces[0]);
        let (s2, _) = ends(&b.pieces[1]);
        assert!(same(e, p(5.0, 5.0)) && same(s2, p(5.0, 5.0)));
        assert!(!same(s, e));
    }
}

#[test]
fn break_line_through_circle() {
    let shapes = vec![
        line(-20.0, 0.0, 20.0, 0.0),
        Shape::Circle {
            center: p(0.0, 0.0),
            radius: 10.0,
        },
    ];
    let (broken, crossings) = break_at_intersections(&shapes);
    assert_eq!(crossings, 2);
    assert_eq!(broken[0].pieces.len(), 3);
    assert!(
        broken[0]
            .pieces
            .iter()
            .all(|s| matches!(s, Shape::Line { .. }))
    );
    let arcs = &broken[1].pieces;
    assert_eq!(arcs.len(), 2);
    let mut angles: Vec<(f64, f64)> = arcs
        .iter()
        .map(|s| match s {
            Shape::Arc {
                start_angle,
                end_angle,
                ..
            } => (*start_angle, *end_angle),
            _ => panic!("not an arc"),
        })
        .collect();
    angles.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert!(
        close(angles[0].0, 0.0) && close(angles[0].1, 180.0),
        "{angles:?}"
    );
    assert!(
        close(angles[1].0, 180.0) && close(angles[1].1, 0.0),
        "{angles:?}"
    );
}

#[test]
fn break_t_junction_cuts_only_the_through_shape() {
    let shapes = vec![line(0.0, 0.0, 20.0, 0.0), line(10.0, 0.0, 10.0, 10.0)];
    let (broken, crossings) = break_at_intersections(&shapes);
    assert_eq!(crossings, 1);
    assert_eq!(broken.len(), 1);
    assert_eq!(broken[0].source, 0);
    assert_eq!(broken[0].pieces.len(), 2);
}

#[test]
fn break_closed_polyline_keeps_arcs() {
    // A 20 x 10 outline with a half-circle bulge on its right side, cut by a vertical line.
    let pl = Shape::Polyline(Polyline {
        vertices: vec![
            v(0.0, 0.0, 0.0),
            v(20.0, 0.0, 1.0),
            v(20.0, 10.0, 0.0),
            v(0.0, 10.0, 0.0),
        ],
        closed: true,
    });
    let shapes = vec![pl, line(10.0, -5.0, 10.0, 15.0)];
    let (broken, _) = break_at_intersections(&shapes);
    let outline = broken.iter().find(|b| b.source == 0).unwrap();
    assert_eq!(outline.pieces.len(), 2);
    let with_arc = outline
        .pieces
        .iter()
        .map(polyline)
        .filter(|pl| pl.vertices.iter().any(|v| close(v.bulge, 1.0)))
        .count();
    assert_eq!(with_arc, 1);
    assert!(outline.pieces.iter().all(|s| !polyline(s).closed));

    // Joining the pieces back gives the outline again.
    let joined = join(&outline.pieces, 1e-6);
    assert_eq!(joined.len(), 1);
    assert!(joined[0].polyline.closed);
}

#[test]
fn break_closed_polyline_cut_once_opens_it() {
    let square = Shape::Polyline(Polyline::rectangle(0.0, 0.0, 10.0, 10.0));
    let shapes = vec![square, line(5.0, -5.0, 5.0, 0.0)]; // touches the bottom edge
    let (broken, _) = break_at_intersections(&shapes);
    assert_eq!(broken.len(), 1);
    let pl = polyline(&broken[0].pieces[0]);
    assert!(!pl.closed);
    assert_eq!(pl.vertices.len(), 6);
    assert!(same(pl.vertices[0].point(), p(5.0, 0.0)));
    assert!(same(pl.vertices[5].point(), p(5.0, 0.0)));
}

#[test]
fn break_circle_once_stays_whole_and_rectangle_becomes_polylines() {
    let circle = Shape::Circle {
        center: p(0.0, 0.0),
        radius: 5.0,
    };
    // Tangent line touches the circle once.
    let (broken, _) = break_at_intersections(&[circle, line(-10.0, 5.0, 10.0, 5.0)]);
    assert!(broken.iter().all(|b| b.source != 0));

    let rect = Shape::rectangle(p(0.0, 0.0), 10.0, 10.0, 0.0);
    let (mut proj, ids) = project_with(vec![rect, line(-5.0, 5.0, 15.0, 5.0)]);
    let report = proj.break_at_intersections(&ids);
    assert_eq!(report.crossings, 2);
    assert_eq!(report.broken, 2);
    assert_eq!(report.ids.len(), 5); // 2 polylines + 3 lines
    assert!(proj.undo());
    assert_eq!(shapes_of(&proj).len(), 2);
}
