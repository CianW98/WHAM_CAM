//! Transform and clipboard tests: move, rotate, scale, mirror, bounds, cut/copy/paste.

use wham_engine::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn assert_point(a: Point, x: f64, y: f64) {
    assert!(
        close(a.x, x) && close(a.y, y),
        "expected ({x}, {y}), got {a:?}"
    );
}

fn assert_bounds(b: Option<[f64; 4]>, want: [f64; 4]) {
    let b = b.expect("no bounds");
    assert!(
        b.iter().zip(want).all(|(a, w)| close(*a, w)),
        "expected {want:?}, got {b:?}"
    );
}

fn line(x0: f64, y0: f64, x1: f64, y1: f64) -> Shape {
    Shape::Line {
        start: p(x0, y0),
        end: p(x1, y1),
    }
}

/// A project with the given shapes on "Default"; returns the project and their ids.
fn project(shapes: Vec<Shape>) -> (Project, Vec<u32>) {
    let mut pr = Project::new();
    let ids = shapes
        .into_iter()
        .map(|s| pr.add_shape("Default", s).unwrap())
        .collect();
    (pr, ids)
}

fn shape(pr: &Project, id: u32) -> Shape {
    pr.document()
        .objects()
        .find(|o| o.id == id)
        .unwrap()
        .shape
        .clone()
}

// --- Bounds ----------------------------------------------------------------------------------

#[test]
fn arc_bounds_include_the_axis_points_it_passes() {
    // Quarter arc from 45° to 135°: passes the top (90°) only.
    let arc = Shape::Arc {
        center: p(0.0, 0.0),
        radius: 10.0,
        start_angle: 45.0,
        end_angle: 135.0,
    };
    let h = 10.0 / 2f64.sqrt();
    assert_bounds(shape_bounds(&arc), [-h, h, h, 10.0]);
    // Long arc from 270° to 540° (= 180°): passes 360° (right) and 450° (top).
    let arc = Shape::Arc {
        center: p(0.0, 0.0),
        radius: 10.0,
        start_angle: 270.0,
        end_angle: 540.0,
    };
    assert_bounds(shape_bounds(&arc), [-10.0, -10.0, 10.0, 10.0]);
}

#[test]
fn polyline_bulge_bounds() {
    // Half circle CCW from (0,0) to (10,0) bulges below: down to y = -5.
    let pl = Shape::Polyline(Polyline {
        vertices: vec![
            Vertex {
                x: 0.0,
                y: 0.0,
                bulge: 1.0,
            },
            Vertex::line(10.0, 0.0),
        ],
        closed: false,
    });
    assert_bounds(shape_bounds(&pl), [0.0, -5.0, 10.0, 0.0]);
}

// --- Transforms ------------------------------------------------------------------------------

#[test]
fn move_shifts_everything() {
    let (mut pr, ids) = project(vec![line(0.0, 0.0, 10.0, 0.0)]);
    pr.transform(&ids, TransformOp::Move { dx: 5.0, dy: -2.0 })
        .unwrap();
    assert_eq!(shape(&pr, ids[0]), line(5.0, -2.0, 15.0, -2.0));
}

#[test]
fn rotate_turns_around_the_selection_centre() {
    // Line from (0,0) to (10,0): centre (5,0). Rotated 90° CCW: (5,-5) to (5,5).
    let (mut pr, ids) = project(vec![line(0.0, 0.0, 10.0, 0.0)]);
    pr.transform(&ids, TransformOp::Rotate { degrees: 90.0 })
        .unwrap();
    let Shape::Line { start, end } = shape(&pr, ids[0]) else {
        panic!()
    };
    assert_point(start, 5.0, -5.0);
    assert_point(end, 5.0, 5.0);
}

#[test]
fn rotate_by_clicking_turns_by_the_angle_between_the_clicks() {
    // Line (0,0)-(10,0), centre (5,0). Reference straight right, then click straight up: 90°.
    let (mut pr, ids) = project(vec![line(0.0, 0.0, 10.0, 0.0)]);
    assert_eq!(pr.selection_centre(&ids), Some(p(5.0, 0.0)));
    let op = TransformOp::RotateFromTo {
        from: p(20.0, 0.0),
        to: p(5.0, 7.0),
    };
    let preview = pr.transform_preview(&ids, op);
    assert_eq!(preview.len(), 1);
    pr.transform(&ids, op).unwrap();
    let Shape::Line { start, end } = shape(&pr, ids[0]) else {
        panic!()
    };
    assert_point(start, 5.0, -5.0);
    assert_point(end, 5.0, 5.0);
}

#[test]
fn rotate_by_clicking_measures_clockwise_as_negative() {
    use wham_engine::transform::angle_from_to;
    let c = p(1.0, 1.0);
    assert!(close(angle_from_to(c, p(2.0, 1.0), p(1.0, 0.0)), -90.0));
    assert!(close(
        angle_from_to(c, p(2.0, 1.0), p(0.0, 1.0)).abs(),
        180.0
    ));
    // A click on the centre has no direction: no turn.
    assert_eq!(angle_from_to(c, c, p(5.0, 5.0)), 0.0);
    assert!(Project::new().selection_centre(&[1]).is_none());
}

#[test]
fn scale_grows_around_the_selection_centre() {
    let (mut pr, ids) = project(vec![
        Shape::Circle {
            center: p(0.0, 0.0),
            radius: 5.0,
        },
        Shape::Circle {
            center: p(20.0, 0.0),
            radius: 5.0,
        },
    ]);
    // Combined centre (10,0). Doubling puts the circles at -10 and 30, radius 10.
    pr.transform(&ids, TransformOp::Scale { factor: 2.0 })
        .unwrap();
    assert_eq!(
        shape(&pr, ids[0]),
        Shape::Circle {
            center: p(-10.0, 0.0),
            radius: 10.0
        }
    );
    assert_eq!(
        shape(&pr, ids[1]),
        Shape::Circle {
            center: p(30.0, 0.0),
            radius: 10.0
        }
    );
}

#[test]
fn mirrored_arc_stays_counter_clockwise_and_in_place() {
    // Arc from 0° to 90° around (0,0), radius 10: spans x 0..10, y 0..10.
    let arc = Shape::Arc {
        center: p(0.0, 0.0),
        radius: 10.0,
        start_angle: 0.0,
        end_angle: 90.0,
    };
    let (mut pr, ids) = project(vec![arc.clone()]);
    pr.transform(&ids, TransformOp::MirrorLeftRight).unwrap();
    let mirrored = shape(&pr, ids[0]);
    // Mirrored across x = 5: centre (10,0), running CCW from 90° to 180°.
    let Shape::Arc {
        center,
        radius,
        start_angle,
        end_angle,
    } = mirrored
    else {
        panic!()
    };
    assert_point(center, 10.0, 0.0);
    assert!(close(radius, 10.0));
    assert!(close(start_angle, 90.0) && close(end_angle, 180.0));
    assert_bounds(shape_bounds(&mirrored), [0.0, 0.0, 10.0, 10.0]);
}

#[test]
fn mirror_top_bottom_flips_polyline_bulges() {
    let pl = Shape::Polyline(Polyline {
        vertices: vec![
            Vertex {
                x: 0.0,
                y: 0.0,
                bulge: 1.0,
            },
            Vertex::line(10.0, 0.0),
        ],
        closed: false,
    });
    let (mut pr, ids) = project(vec![pl]);
    pr.transform(&ids, TransformOp::MirrorTopBottom).unwrap();
    let mirrored = shape(&pr, ids[0]);
    let Shape::Polyline(m) = &mirrored else {
        panic!()
    };
    assert_eq!(m.vertices[0].bulge, -1.0);
    // Was y -5..0, centre y -2.5: mirrored it is still y -5..0, now bulging the other way.
    assert_bounds(shape_bounds(&mirrored), [0.0, -5.0, 10.0, 0.0]);
    assert_point(m.vertices[0].point(), 0.0, -5.0);
}

#[test]
fn bad_values_change_nothing() {
    let (mut pr, ids) = project(vec![line(0.0, 0.0, 10.0, 0.0)]);
    for op in [
        TransformOp::Scale { factor: 0.0 },
        TransformOp::Scale { factor: -1.0 },
        TransformOp::Rotate { degrees: f64::NAN },
        TransformOp::Move {
            dx: f64::INFINITY,
            dy: 0.0,
        },
    ] {
        assert!(pr.transform(&ids, op).is_err(), "{op:?} should be refused");
    }
    assert_eq!(shape(&pr, ids[0]), line(0.0, 0.0, 10.0, 0.0));
    assert!(pr.can_undo()); // only the add_shape
    pr.undo();
    assert!(!pr.can_undo());
}

#[test]
fn transform_is_one_undo_step_and_preview_changes_nothing() {
    let (mut pr, ids) = project(vec![line(0.0, 0.0, 10.0, 0.0), line(0.0, 5.0, 10.0, 5.0)]);
    let preview = pr.transform_preview(&ids, TransformOp::Move { dx: 1.0, dy: 0.0 });
    assert_eq!(preview.len(), 2);
    assert_eq!(preview[0].points[0], [1.0, 0.0]);
    assert_eq!(shape(&pr, ids[0]), line(0.0, 0.0, 10.0, 0.0));
    pr.transform(&ids, TransformOp::Rotate { degrees: 30.0 })
        .unwrap();
    pr.undo();
    assert_eq!(shape(&pr, ids[0]), line(0.0, 0.0, 10.0, 0.0));
    assert_eq!(shape(&pr, ids[1]), line(0.0, 5.0, 10.0, 5.0));
}

// --- Clipboard -------------------------------------------------------------------------------

#[test]
fn copy_and_paste_puts_the_centre_at_the_point_on_the_original_layer() {
    let mut pr = Project::new();
    let id = pr.add_shape("CUT", line(0.0, 0.0, 10.0, 0.0)).unwrap();
    assert!(!pr.has_clipboard());
    assert_eq!(pr.copy(&[id]), 1);
    let new = pr.paste(p(100.0, 50.0));
    assert_eq!(new.len(), 1);
    assert_ne!(new[0], id);
    assert_eq!(shape(&pr, new[0]), line(95.0, 50.0, 105.0, 50.0));
    assert_eq!(pr.ids_on_layer("CUT"), vec![id, new[0]]);
    // Paste again: the clipboard is kept.
    let again = pr.paste(p(0.0, 0.0));
    assert_eq!(shape(&pr, again[0]), line(-5.0, 0.0, 5.0, 0.0));
    // Each paste is one undo step.
    pr.undo();
    assert_eq!(pr.all_ids().len(), 2);
}

#[test]
fn paste_preview_changes_nothing() {
    let (mut pr, ids) = project(vec![line(0.0, 0.0, 10.0, 0.0)]);
    pr.copy(&ids);
    let preview = pr.paste_preview(p(5.0, 10.0));
    assert_eq!(preview[0].points, vec![[0.0, 10.0], [10.0, 10.0]]);
    assert_eq!(pr.all_ids().len(), 1);
}

#[test]
fn cut_deletes_in_one_undo_step_and_keeps_the_clipboard() {
    let (mut pr, ids) = project(vec![line(0.0, 0.0, 10.0, 0.0)]);
    assert_eq!(pr.cut(&ids), 1);
    assert!(pr.all_ids().is_empty());
    pr.undo();
    assert_eq!(pr.all_ids(), ids);
    assert!(pr.has_clipboard());
}

#[test]
fn copying_nothing_keeps_the_old_clipboard() {
    let (mut pr, ids) = project(vec![line(0.0, 0.0, 10.0, 0.0)]);
    pr.copy(&ids);
    assert_eq!(pr.copy(&[]), 1);
    assert_eq!(pr.cut(&[999]), 0);
    assert!(pr.has_clipboard());
}

#[test]
fn select_all_helpers() {
    let mut pr = Project::new();
    let a = pr.add_shape("A", line(0.0, 0.0, 1.0, 0.0)).unwrap();
    let b = pr.add_shape("B", line(0.0, 1.0, 1.0, 1.0)).unwrap();
    assert_eq!(pr.all_ids(), vec![a, b]);
    assert_eq!(pr.ids_on_layer("B"), vec![b]);
    assert!(pr.ids_on_layer("nope").is_empty());
}

// --- PolyRectangle ---------------------------------------------------------------------------

fn rect(x: f64, y: f64, w: f64, h: f64, rotation: f64) -> Shape {
    Shape::Rectangle {
        lower_left: p(x, y),
        width: w,
        height: h,
        rotation,
    }
}

fn assert_rect(s: &Shape, x: f64, y: f64, w: f64, h: f64, rotation: f64) {
    let Shape::Rectangle {
        lower_left,
        width,
        height,
        rotation: r,
    } = s
    else {
        panic!("not a rectangle: {s:?}");
    };
    assert_point(*lower_left, x, y);
    assert!(
        close(*width, w) && close(*height, h) && close(*r, rotation),
        "expected {w} x {h} at {rotation}°, got {s:?}"
    );
}

#[test]
fn rectangle_move_and_scale() {
    let (mut pr, ids) = project(vec![rect(0.0, 0.0, 20.0, 10.0, 0.0)]);
    pr.transform(&ids, TransformOp::Move { dx: 5.0, dy: 5.0 })
        .unwrap();
    assert_rect(&shape(&pr, ids[0]), 5.0, 5.0, 20.0, 10.0, 0.0);
    // Centre (15,10); doubling: lower left (-5,0), 40 x 20.
    pr.transform(&ids, TransformOp::Scale { factor: 2.0 })
        .unwrap();
    assert_rect(&shape(&pr, ids[0]), -5.0, 0.0, 40.0, 20.0, 0.0);
}

#[test]
fn rectangle_rotated_by_an_odd_angle_keeps_a_rotation() {
    let (mut pr, ids) = project(vec![rect(0.0, 0.0, 20.0, 10.0, 0.0)]);
    pr.transform(&ids, TransformOp::Rotate { degrees: 30.0 })
        .unwrap();
    let s = shape(&pr, ids[0]);
    assert_eq!(s.type_name(), "PolyRectangle");
    let Shape::Rectangle {
        width,
        height,
        rotation,
        ..
    } = s
    else {
        panic!()
    };
    assert!(close(width, 20.0) && close(height, 10.0) && close(rotation, 30.0));
    // Turning back again gives the original rectangle.
    pr.transform(&ids, TransformOp::Rotate { degrees: -30.0 })
        .unwrap();
    assert_rect(&shape(&pr, ids[0]), 0.0, 0.0, 20.0, 10.0, 0.0);
}

#[test]
fn rectangle_rotated_90_is_stored_square_to_the_axes() {
    // 20 x 10 at (0,0), centre (10,5). Turned 90°: 10 wide, 20 high, lower left (5,-5).
    let (mut pr, ids) = project(vec![rect(0.0, 0.0, 20.0, 10.0, 0.0)]);
    pr.transform(&ids, TransformOp::Rotate { degrees: 90.0 })
        .unwrap();
    assert_rect(&shape(&pr, ids[0]), 5.0, -5.0, 10.0, 20.0, 0.0);
}

#[test]
fn rectangle_mirrors_stay_rectangles() {
    let (mut pr, ids) = project(vec![rect(0.0, 0.0, 20.0, 10.0, 0.0)]);
    pr.transform(&ids, TransformOp::MirrorLeftRight).unwrap();
    assert_rect(&shape(&pr, ids[0]), 0.0, 0.0, 20.0, 10.0, 0.0);
    // A rotated rectangle mirrored left-right turns the other way.
    let (mut pr, ids) = project(vec![rect(0.0, 0.0, 20.0, 10.0, 30.0)]);
    pr.transform(&ids, TransformOp::MirrorLeftRight).unwrap();
    let Shape::Rectangle {
        width,
        height,
        rotation,
        ..
    } = shape(&pr, ids[0])
    else {
        panic!()
    };
    assert!(close(width, 20.0) && close(height, 10.0) && close(rotation, 330.0));
    assert_eq!(pr.display().shapes[0].direction, Some(Direction::CCW));
}

#[test]
fn rectangle_bounds_follow_rotation() {
    let b = shape_bounds(&rect(0.0, 0.0, 10.0, 10.0, 45.0));
    let d = 10.0 / 2f64.sqrt();
    assert_bounds(b, [-d, 0.0, d, 2.0 * d]);
}

#[test]
fn closed_polyline_direction() {
    let ccw = Shape::Polyline(Polyline::rectangle(0.0, 0.0, 10.0, 10.0));
    let mut cw = Polyline::rectangle(0.0, 0.0, 10.0, 10.0);
    cw.vertices.reverse();
    let open = Shape::Polyline(Polyline {
        vertices: vec![Vertex::line(0.0, 0.0), Vertex::line(5.0, 0.0)],
        closed: false,
    });
    let (pr, _) = project(vec![ccw, Shape::Polyline(cw), open]);
    let dirs: Vec<_> = pr.display().shapes.iter().map(|s| s.direction).collect();
    assert_eq!(dirs, vec![Some(Direction::CCW), Some(Direction::CW), None]);
}

#[test]
fn polyline_direction_counts_bulge_arcs() {
    // Two points joined by two half circles: CW bulges (-1) give a clockwise circle.
    let circle = |b: f64| {
        Shape::Polyline(Polyline {
            vertices: vec![
                Vertex {
                    x: 0.0,
                    y: 0.0,
                    bulge: b,
                },
                Vertex {
                    x: 10.0,
                    y: 0.0,
                    bulge: b,
                },
            ],
            closed: true,
        })
    };
    let (pr, _) = project(vec![circle(-1.0), circle(1.0)]);
    let dirs: Vec<_> = pr.display().shapes.iter().map(|s| s.direction).collect();
    assert_eq!(dirs, vec![Some(Direction::CW), Some(Direction::CCW)]);
}
