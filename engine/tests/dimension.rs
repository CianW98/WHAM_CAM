//! Dimensions (spec 5.3, decided 2026-11-20): their own list (not on layers), attached to the
//! geometry they measure, placed and dragged freely, deleted with what they measure, and sized
//! for the screen.

use wham_engine::geometry::{Polyline, Vertex};
use wham_engine::project_file::{from_text, to_text};
use wham_engine::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

const SNAPS: SnapOptions = SnapOptions {
    grid: true,
    objects: true,
};

fn new_draft() -> Draft {
    Draft {
        tool: DrawTool::Dimension,
        points: Vec::new(),
        arcs: Vec::new(),
        arc_next: false,
        targets: Vec::new(),
    }
}

/// Clicks with the Dimension tool as the interface does (0.5 mm click radius), until a click
/// finishes it; then adds it. Returns the new dimension's id.
fn dimension_by_clicks(proj: &mut Project, clicks: &[Point]) -> Result<u32, String> {
    let mut d = new_draft();
    for (i, c) in clicks.iter().enumerate() {
        let r = proj.dimension_click(&d, c.x, c.y, 0.5, SNAPS);
        if let Some(problem) = r.problem {
            return Err(problem);
        }
        d.points.push(r.point);
        d.targets.push(r.target);
        assert_eq!(r.finishes, i + 1 == clicks.len(), "click {i} at {c:?}");
    }
    proj.add_draft("Default", &d)
}

/// A dimension as it is now.
fn info(proj: &Project, id: u32) -> DimensionInfo {
    proj.dimensions()
        .into_iter()
        .find(|d| d.id == id)
        .expect("the dimension exists")
}

/// A dimension's stored definition.
fn def(proj: &Project, id: u32) -> DimensionDef {
    proj.document()
        .dimensions
        .iter()
        .find(|d| d.id == id)
        .map(|d| d.def.clone())
        .expect("the dimension exists")
}

fn line(proj: &mut Project, a: Point, b: Point) -> u32 {
    proj.add_shape("Default", Shape::Line { start: a, end: b })
        .unwrap()
}

fn circle(proj: &mut Project, c: Point, r: f64) -> u32 {
    proj.add_shape(
        "Default",
        Shape::Circle {
            center: c,
            radius: r,
        },
    )
    .unwrap()
}

fn label_at(proj: &Project, id: u32) -> [f64; 2] {
    proj.display()
        .shapes
        .iter()
        .find(|s| s.id == id)
        .and_then(|s| s.label.clone())
        .expect("drawn with a label")
        .at
}

#[test]
fn dimensions_are_not_on_any_layer() {
    let mut proj = Project::new();
    circle(&mut proj, p(10.0, 10.0), 12.5);
    let id = dimension_by_clicks(&mut proj, &[p(22.4, 10.0), p(40.0, 20.0)]).unwrap();
    assert_eq!(
        proj.document().objects().count(),
        1,
        "only the circle is a shape"
    );
    assert_eq!(proj.document().dimensions.len(), 1);
    assert_eq!(info(&proj, id).text, "Ø25.00");
    assert_eq!(info(&proj, id).kind, DimensionKind::Diameter);
}

#[test]
fn an_arc_gets_its_radius_even_clicked_at_its_midpoint() {
    let mut proj = Project::new();
    proj.add_shape(
        "Default",
        Shape::Arc {
            center: p(0.0, 0.0),
            radius: 20.0,
            start_angle: 0.0,
            end_angle: 90.0,
        },
    )
    .unwrap();
    let mid = p(
        20.0 * 45f64.to_radians().cos(),
        20.0 * 45f64.to_radians().sin(),
    );
    let id = dimension_by_clicks(&mut proj, &[mid, p(30.0, 30.0)]).unwrap();
    assert_eq!(info(&proj, id).text, "R20.00");
}

#[test]
fn polyline_segments_and_rectangle_sides() {
    // The arc test shape: convex arc R37.5 on the right, concave arc R72.5 on top.
    let mut proj = Project::new();
    let v = |x: f64, y: f64, bulge: f64| Vertex { x, y, bulge };
    proj.add_shape(
        "Default",
        Shape::Polyline(Polyline {
            vertices: vec![
                v(0.0, 0.0, 0.0),
                v(100.0, 0.0, 0.5),
                v(100.0, 60.0, -0.4),
                v(0.0, 60.0, 0.0),
            ],
            closed: true,
        }),
    )
    .unwrap();
    let r = dimension_by_clicks(&mut proj, &[p(115.0, 30.0), p(130.0, 30.0)]).unwrap();
    assert!(close(info(&proj, r).value, 37.5));
    let top = dimension_by_clicks(&mut proj, &[p(50.0, 40.0), p(50.0, 20.0)]).unwrap();
    assert!(close(info(&proj, top).value, 72.5));
    let side = dimension_by_clicks(&mut proj, &[p(50.0, 0.0), p(50.0, -10.0)]).unwrap();
    assert!(close(info(&proj, side).value, 100.0));
    // Bottom and left sides: 90° between them.
    let a = dimension_by_clicks(&mut proj, &[p(50.0, 0.0), p(0.0, 30.0), p(10.0, 10.0)]).unwrap();
    assert!(close(info(&proj, a).value, 90.0));
}

#[test]
fn parallel_lines_get_the_distance_between_them() {
    let mut proj = Project::new();
    line(&mut proj, p(0.0, 0.0), p(10.0, 0.0));
    line(&mut proj, p(2.0, 7.5), p(20.0, 7.5));
    let id = dimension_by_clicks(&mut proj, &[p(5.0, 0.0), p(12.0, 7.5), p(-5.0, 3.0)]).unwrap();
    assert!(close(info(&proj, id).value, 7.5));
}

#[test]
fn two_lines_get_the_angle_the_last_click_is_in() {
    let mut proj = Project::new();
    line(&mut proj, p(0.0, 0.0), p(10.0, 0.0));
    line(&mut proj, p(0.0, 0.0), p(10.0, 10.0));
    let mut angle_at = |at: Point| {
        let id = dimension_by_clicks(&mut proj, &[p(5.0, 0.0), p(5.0, 5.0), at]).unwrap();
        info(&proj, id).value
    };
    assert!(close(angle_at(p(8.0, 3.0)), 45.0));
    assert!(close(angle_at(p(-1.0, 8.0)), 135.0));
    assert!(close(angle_at(p(-8.0, -3.0)), 45.0));
    assert!(close(angle_at(p(1.0, -8.0)), 135.0));
}

#[test]
fn points_must_be_on_shapes_and_the_placing_click_is_free() {
    let mut proj = Project::new();
    line(&mut proj, p(0.0, 0.0), p(10.0, 0.0));
    line(&mut proj, p(30.0, 40.0), p(30.0, 50.0));
    // Empty space can't start a dimension, even on a grid point.
    let r = proj.dimension_click(&new_draft(), 50.0, 50.0, 0.5, SNAPS);
    assert!(r.problem.is_some());
    // An end, then empty space for the second point: refused.
    let err = dimension_by_clicks(&mut proj, &[p(0.1, 0.0), p(20.0, 20.0)]).unwrap_err();
    assert!(err.contains("end, midpoint or centre"), "{err}");
    // Two ends (snapped exactly), then a placing click that isn't on any grid point: used
    // exactly as clicked.
    let mut d = new_draft();
    for c in [p(0.1, 0.0), p(30.1, 39.9)] {
        let r = proj.dimension_click(&d, c.x, c.y, 0.5, SNAPS);
        d.points.push(r.point);
        d.targets.push(r.target);
    }
    assert_eq!(d.points, [p(0.0, 0.0), p(30.0, 40.0)]);
    let r = proj.dimension_click(&d, -8.03, 6.07, 0.5, SNAPS);
    assert_eq!(r.point, p(-8.03, 6.07));
    assert_eq!(r.kind, SnapKind::None);
    assert!(r.target.is_none() && r.finishes);
    // A circle's centre is a point, not the circle.
    circle(&mut proj, p(60.0, 0.0), 5.0);
    let r = proj.dimension_click(&new_draft(), 60.1, 0.0, 0.5, SNAPS);
    assert!(matches!(r.target, Some(Target::Point(_))));
    assert_eq!(r.kind, SnapKind::Centre);
}

#[test]
fn dimensions_follow_their_shapes() {
    let mut proj = Project::new();
    let c = circle(&mut proj, p(0.0, 0.0), 5.0);
    let a = line(&mut proj, p(20.0, 0.0), p(30.0, 0.0));
    let b = line(&mut proj, p(40.0, 0.0), p(50.0, 0.0));
    let dia = dimension_by_clicks(&mut proj, &[p(5.0, 0.1), p(10.0, 10.0)]).unwrap();
    // Between the end of one line and the start of the other: 10 apart.
    let gap = dimension_by_clicks(&mut proj, &[p(30.0, 0.0), p(40.0, 0.0), p(35.0, 5.0)]).unwrap();
    assert!(close(info(&proj, gap).value, 10.0));
    // Scale the circle: the diameter follows.
    proj.transform(&[c], TransformOp::Scale { factor: 2.0 })
        .unwrap();
    assert_eq!(info(&proj, dia).text, "Ø20.00");
    // Move one line: the distance follows.
    proj.transform(&[b], TransformOp::Move { dx: 5.0, dy: 0.0 })
        .unwrap();
    assert!(close(info(&proj, gap).value, 15.0));
    // Edit the other in Properties: still attached.
    proj.set_shape(
        a,
        Shape::Line {
            start: p(20.0, 0.0),
            end: p(32.0, 0.0),
        },
    )
    .unwrap();
    assert!(close(info(&proj, gap).value, 13.0));
}

#[test]
fn moving_a_part_carries_its_dimension_along() {
    let mut proj = Project::new();
    let l = line(&mut proj, p(0.0, 0.0), p(40.0, 0.0));
    let id = dimension_by_clicks(&mut proj, &[p(20.0, 0.0), p(20.0, -10.0)]).unwrap();
    let before = label_at(&proj, id);
    proj.transform(&[l], TransformOp::Move { dx: 7.0, dy: 3.0 })
        .unwrap();
    let after = label_at(&proj, id);
    assert!(close(after[0] - before[0], 7.0) && close(after[1] - before[1], 3.0));
    // Rotating it: the dimension line stays the same distance from the line, on the same side.
    proj.transform(&[l], TransformOp::Rotate { degrees: 90.0 })
        .unwrap();
    let DimensionDef::Length { offset, .. } = def(&proj, id) else {
        panic!()
    };
    assert!(close(offset, -10.0));
}

#[test]
fn a_dimension_goes_when_what_it_measures_goes() {
    let mut proj = Project::new();
    let c = circle(&mut proj, p(0.0, 0.0), 5.0);
    dimension_by_clicks(&mut proj, &[p(5.0, 0.1), p(10.0, 0.0)]).unwrap();
    proj.delete(&[c]);
    assert!(proj.document().dimensions.is_empty());
    // Undo brings both back in one step.
    assert!(proj.undo());
    assert_eq!(proj.document().dimensions.len(), 1);
    // Exploding a rectangle replaces it with lines: its dimension goes.
    let r = proj
        .add_shape("Default", Shape::rectangle(p(20.0, 0.0), 10.0, 5.0, 0.0))
        .unwrap();
    dimension_by_clicks(&mut proj, &[p(25.0, 0.0), p(25.0, -5.0)]).unwrap();
    assert_eq!(proj.document().dimensions.len(), 2);
    proj.explode(&[r]);
    assert_eq!(proj.document().dimensions.len(), 1);
}

#[test]
fn a_polyline_losing_the_measured_segment_loses_the_dimension() {
    let mut proj = Project::new();
    let pl = proj
        .add_shape(
            "Default",
            Shape::Polyline(Polyline {
                vertices: vec![
                    Vertex::line(0.0, 0.0),
                    Vertex::line(10.0, 0.0),
                    Vertex::line(10.0, 10.0),
                ],
                closed: false,
            }),
        )
        .unwrap();
    // The second segment's length.
    dimension_by_clicks(&mut proj, &[p(10.0, 5.0), p(15.0, 5.0)]).unwrap();
    proj.set_shape(
        pl,
        Shape::Polyline(Polyline {
            vertices: vec![Vertex::line(0.0, 0.0), Vertex::line(10.0, 0.0)],
            closed: false,
        }),
    )
    .unwrap();
    assert!(proj.document().dimensions.is_empty());
}

#[test]
fn dragging_moves_where_it_sits_as_one_undo_step() {
    let mut proj = Project::new();
    line(&mut proj, p(0.0, 0.0), p(40.0, 0.0));
    let id = dimension_by_clicks(&mut proj, &[p(20.0, 0.0), p(20.0, -10.0)]).unwrap();
    // Grab it by its dimension line (y = −10), not the line it measures.
    assert_eq!(proj.dimension_at(30.0, -10.0, 0.5), Some(id));
    assert_eq!(proj.dimension_at(30.0, 0.0, 0.5), None);
    assert!(proj.dimension_drag_preview(id, 30.0, 25.0).is_some());
    proj.reposition_dimension(id, 30.0, 25.0).unwrap();
    let DimensionDef::Length { offset, .. } = def(&proj, id) else {
        panic!()
    };
    assert!(close(offset, 25.0));
    assert!(
        close(info(&proj, id).value, 40.0),
        "the value doesn't change"
    );
    proj.undo();
    let DimensionDef::Length { offset, .. } = def(&proj, id) else {
        panic!()
    };
    assert!(close(offset, -10.0));
}

#[test]
fn text_and_arrows_keep_their_size_on_screen() {
    let mut proj = Project::new();
    line(&mut proj, p(0.0, 0.0), p(40.0, 0.0));
    let id = dimension_by_clicks(&mut proj, &[p(20.0, 0.0), p(20.0, -10.0)]).unwrap();
    let height = |proj: &Project| {
        proj.display()
            .shapes
            .iter()
            .find(|s| s.id == id)
            .and_then(|s| s.label.clone())
            .unwrap()
            .height
    };
    proj.set_view_scale(0.1);
    let near = height(&proj);
    proj.set_view_scale(1.0);
    let far = height(&proj);
    assert!(close(far, near * 10.0), "{near} vs {far}");
}

#[test]
fn dimensions_can_be_hidden_together_and_are_selectable() {
    let mut proj = Project::new();
    let l = line(&mut proj, p(0.0, 0.0), p(40.0, 0.0));
    let id = dimension_by_clicks(&mut proj, &[p(20.0, 0.0), p(20.0, -10.0)]).unwrap();
    assert_eq!(proj.all_ids(), [l, id]);
    assert_eq!(proj.pick(30.0, -10.0, 0.5), Some(id));
    proj.set_dimensions_visible(false);
    assert_eq!(proj.all_ids(), [l]);
    assert_eq!(proj.pick(30.0, -10.0, 0.5), None);
    assert!(proj.display().shapes.iter().all(|s| s.id != id));
}

#[test]
fn dimensions_are_ignored_by_transforms_copy_and_operations() {
    let mut proj = Project::new();
    let c = circle(&mut proj, p(0.0, 0.0), 5.0);
    let d = dimension_by_clicks(&mut proj, &[p(5.0, 0.1), p(10.0, 0.0)]).unwrap();
    // Moving the dimension alone does nothing to it (it follows its circle).
    proj.transform(&[d], TransformOp::Move { dx: 50.0, dy: 0.0 })
        .unwrap();
    assert_eq!(info(&proj, d).text, "Ø10.00");
    assert_eq!(proj.copy(&[d]), 0);
    assert!(proj.add_pocket(&[d]).is_err());
    assert!(proj.add_drill(&[c, d]).is_err());
}

#[test]
fn dimensions_are_saved_and_opened() {
    let mut proj = Project::new();
    circle(&mut proj, p(0.0, 0.0), 5.0);
    line(&mut proj, p(20.0, 0.0), p(24.0, 3.0));
    dimension_by_clicks(&mut proj, &[p(5.0, 0.1), p(10.0, 3.0)]).unwrap();
    dimension_by_clicks(&mut proj, &[p(20.0, 0.0), p(24.0, 3.0), p(29.0, 9.0)]).unwrap();
    let text = to_text(proj.document());
    assert_eq!(&from_text(&text).unwrap(), proj.document());
}
