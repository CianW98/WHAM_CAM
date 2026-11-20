//! Dimensions (spec 5.3): the one Dimension tool working out the kind from what is clicked,
//! the measured values, picking, transforms, saving, and operations refusing them.

use wham_engine::geometry::{Polyline, Vertex};
use wham_engine::project_file::{FORMAT_VERSION, from_text, to_text};
use wham_engine::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

const SNAPS: SnapOptions = SnapOptions {
    grid: false,
    objects: true,
};

fn new_draft() -> Draft {
    Draft {
        tool: DrawTool::Dimension,
        points: Vec::new(),
        arcs: Vec::new(),
        arc_next: false,
        picks: Vec::new(),
    }
}

/// Clicks with the Dimension tool as the interface does (0.5 mm click radius), until a click
/// finishes it; then adds it. Returns the new dimension's id.
fn dimension_by_clicks(proj: &mut Project, clicks: &[Point]) -> Result<u32, String> {
    let mut d = new_draft();
    for (i, c) in clicks.iter().enumerate() {
        let r = proj.dimension_click(&d, c.x, c.y, 0.5, SNAPS);
        d.points.push(r.point);
        d.picks.push(r.pick);
        assert_eq!(
            r.finishes,
            i + 1 == clicks.len(),
            "click {i} at {c:?}: {r:?}"
        );
    }
    proj.add_draft("Default", &d)
}

fn dimension(proj: &Project, id: u32) -> Dimension {
    match proj
        .document()
        .objects()
        .find(|o| o.id == id)
        .map(|o| &o.shape)
    {
        Some(Shape::Dimension(d)) => d.clone(),
        other => panic!("{other:?}"),
    }
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

#[test]
fn a_circle_gets_its_diameter() {
    let mut proj = Project::new();
    circle(&mut proj, p(10.0, 10.0), 12.5);
    // Click on the circle, then where the value goes.
    let id = dimension_by_clicks(&mut proj, &[p(22.4, 10.0), p(40.0, 20.0)]).unwrap();
    assert_eq!(dimension(&proj, id).text(), "Ø25.00");
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
    assert_eq!(dimension(&proj, id).text(), "R20.00");
}

#[test]
fn an_arc_segment_of_a_polyline_gets_its_radius() {
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
    assert!(close(dimension(&proj, r).value(), 37.5));
    let top = dimension_by_clicks(&mut proj, &[p(50.0, 40.0), p(50.0, 20.0)]).unwrap();
    assert!(close(dimension(&proj, top).value(), 72.5));
    // The straight bottom side, then empty space: its length.
    let side = dimension_by_clicks(&mut proj, &[p(50.0, 0.0), p(50.0, -10.0)]).unwrap();
    assert!(close(dimension(&proj, side).value(), 100.0));
}

#[test]
fn two_points_get_an_aligned_dimension_through_the_third_click() {
    let mut proj = Project::new();
    // Ends of two separate lines: clicking at an end is a point, not the line.
    line(&mut proj, p(-10.0, 0.0), p(0.0, 0.0));
    line(&mut proj, p(30.0, 40.0), p(30.0, 50.0));
    let id = dimension_by_clicks(&mut proj, &[p(0.2, 0.1), p(30.1, 39.9), p(-8.0, 6.0)]).unwrap();
    let dim = dimension(&proj, id);
    assert!(close(dim.value(), 50.0));
    assert_eq!(dim.text(), "50.00");
    // The third click is 10 mm to the left of the measured line (direction (0.6, 0.8)), so the
    // dimension line runs from (−8, 6) to (22, 46).
    let shown = display::display_shape(id, &Shape::Dimension(dim));
    let main = shown
        .extra
        .iter()
        .find(|l| l.len() == 2 && close(l[0][0], -8.0) && close(l[0][1], 6.0))
        .expect("dimension line through the third click");
    assert!(
        close(main[1][0], 22.0) && close(main[1][1], 46.0),
        "{main:?}"
    );
    let label = shown.label.unwrap();
    assert!(close(label.angle, 53.13010235415598), "{}", label.angle);
}

#[test]
fn a_circle_centre_is_a_point_not_the_circle() {
    let mut proj = Project::new();
    circle(&mut proj, p(0.0, 0.0), 5.0);
    circle(&mut proj, p(40.0, 0.0), 5.0);
    let id = dimension_by_clicks(&mut proj, &[p(0.1, 0.0), p(40.0, 0.1), p(20.0, 10.0)]).unwrap();
    assert_eq!(dimension(&proj, id).text(), "40.00");
}

#[test]
fn a_line_then_elsewhere_gets_its_length() {
    let mut proj = Project::new();
    line(&mut proj, p(0.0, 0.0), p(30.0, 40.0));
    // Along the line (at its midpoint even), then away from it.
    let id = dimension_by_clicks(&mut proj, &[p(15.0, 20.0), p(0.0, 30.0)]).unwrap();
    assert!(close(dimension(&proj, id).value(), 50.0));
    // Clicking the same line again also places it.
    let again = dimension_by_clicks(&mut proj, &[p(15.0, 20.0), p(24.0, 32.0)]).unwrap();
    assert!(close(dimension(&proj, again).value(), 50.0));
}

#[test]
fn two_lines_get_the_angle_the_last_click_is_in() {
    let mut proj = Project::new();
    line(&mut proj, p(0.0, 0.0), p(10.0, 0.0));
    line(&mut proj, p(0.0, 0.0), p(10.0, 10.0));
    let mut angle_at = |at: Point| {
        let id = dimension_by_clicks(&mut proj, &[p(5.0, 0.0), p(5.0, 5.0), at]).unwrap();
        dimension(&proj, id).value()
    };
    // Between the lines: 45°. Above the slanted line: 135°. The other two are the same.
    assert!(close(angle_at(p(8.0, 3.0)), 45.0));
    assert!(close(angle_at(p(-1.0, 8.0)), 135.0));
    assert!(close(angle_at(p(-8.0, -3.0)), 45.0));
    assert!(close(angle_at(p(1.0, -8.0)), 135.0));
}

#[test]
fn angle_between_a_rectangle_side_and_a_line_that_dont_meet() {
    // The lines are extended to where they would cross.
    let mut proj = Project::new();
    proj.add_shape("Default", Shape::rectangle(p(0.0, 0.0), 20.0, 10.0, 0.0))
        .unwrap();
    line(&mut proj, p(30.0, 5.0), p(40.0, 15.0));
    let id = dimension_by_clicks(&mut proj, &[p(10.0, 0.0), p(35.0, 10.0), p(40.0, 2.0)]).unwrap();
    let Dimension::Angle { vertex, value, .. } = dimension(&proj, id) else {
        panic!()
    };
    assert!(close(vertex.x, 25.0) && close(vertex.y, 0.0), "{vertex:?}");
    assert!(close(value, 45.0));
    assert_eq!(dimension(&proj, id).text(), "45.00°");
}

#[test]
fn parallel_lines_get_the_distance_between_them() {
    let mut proj = Project::new();
    line(&mut proj, p(0.0, 0.0), p(10.0, 0.0));
    line(&mut proj, p(2.0, 7.5), p(20.0, 7.5));
    let id = dimension_by_clicks(&mut proj, &[p(5.0, 0.0), p(12.0, 7.5), p(-5.0, 3.0)]).unwrap();
    let Dimension::Aligned {
        from, to, value, ..
    } = dimension(&proj, id)
    else {
        panic!()
    };
    assert!(close(value, 7.5));
    // Square across from the first click.
    assert!(close(from.x, 5.0) && close(from.y, 0.0) && close(to.x, 5.0) && close(to.y, 7.5));
}

#[test]
fn hovering_a_point_snaps_but_the_first_click_on_a_line_picks_it() {
    let mut proj = Project::new();
    let l = line(&mut proj, p(0.0, 0.0), p(10.0, 0.0));
    let at_end = proj.dimension_click(&new_draft(), 9.8, 0.1, 0.5, SNAPS);
    assert_eq!(
        (at_end.point, at_end.kind, at_end.pick),
        (p(10.0, 0.0), SnapKind::End, None)
    );
    let along = proj.dimension_click(&new_draft(), 5.0, 0.1, 0.5, SNAPS);
    assert_eq!(
        along.pick,
        Some(Pick {
            id: l,
            kind: PickKind::Line
        })
    );
    assert!(!along.finishes);
}

#[test]
fn an_unfinished_dimension_is_refused() {
    let mut proj = Project::new();
    let mut d = new_draft();
    d.points = vec![p(0.0, 0.0), p(10.0, 0.0)];
    d.picks = vec![None, None];
    assert!(proj.add_draft("Default", &d).is_err());
}

#[test]
fn moving_and_mirroring_keep_the_value_and_scaling_scales_lengths() {
    let mut proj = Project::new();
    line(&mut proj, p(0.0, 0.0), p(10.0, 0.0));
    line(&mut proj, p(0.0, 0.0), p(10.0, 10.0));
    // The bottom line's length, and the 45° angle.
    let al = dimension_by_clicks(&mut proj, &[p(5.0, 0.0), p(5.0, -5.0)]).unwrap();
    let an = dimension_by_clicks(&mut proj, &[p(5.0, 0.0), p(5.0, 5.0), p(8.0, 3.0)]).unwrap();
    proj.transform(&[al, an], TransformOp::Move { dx: 7.0, dy: -3.0 })
        .unwrap();
    proj.transform(&[al, an], TransformOp::MirrorLeftRight)
        .unwrap();
    proj.transform(&[al, an], TransformOp::Rotate { degrees: 30.0 })
        .unwrap();
    assert!(close(dimension(&proj, al).value(), 10.0));
    assert!(close(dimension(&proj, an).value(), 45.0));
    proj.transform(&[al, an], TransformOp::Scale { factor: 2.5 })
        .unwrap();
    assert!((dimension(&proj, al).value() - 25.0).abs() < 1e-9);
    assert!((dimension(&proj, an).value() - 45.0).abs() < 1e-9);
    // After the mirror, the angle's arc still runs anticlockwise through the 45° gap.
    let Dimension::Angle {
        vertex, start, end, ..
    } = dimension(&proj, an)
    else {
        panic!()
    };
    let turn = ((end.y - vertex.y).atan2(end.x - vertex.x)
        - (start.y - vertex.y).atan2(start.x - vertex.x))
    .to_degrees()
    .rem_euclid(360.0);
    assert!((turn - 45.0).abs() < 1e-9, "{turn}");
}

#[test]
fn operations_refuse_dimensions() {
    let mut proj = Project::new();
    circle(&mut proj, p(0.0, 0.0), 5.0);
    let d = dimension_by_clicks(&mut proj, &[p(5.0, 0.1), p(10.0, 0.0)]).unwrap();
    for err in [
        proj.add_pocket(&[d]).unwrap_err(),
        proj.add_profile(&[d]).unwrap_err(),
        proj.add_drill(&[d]).unwrap_err(),
    ] {
        assert!(err.contains("never machined"), "{err}");
    }
}

#[test]
fn clicking_the_value_or_a_line_selects_the_dimension_and_snapping_ignores_it() {
    let mut proj = Project::new();
    let id = dimension_by_clicks(&mut proj, &[p(0.0, 0.0), p(40.0, 0.0), p(20.0, -10.0)]).unwrap();
    let shown = display::display_shape(id, &proj.document().objects().next().unwrap().shape);
    let label = shown.label.unwrap();
    // The value sits below the dimension line (y = −10), centred at x = 20.
    assert!(
        close(label.at[0], 20.0) && label.at[1] < -10.0,
        "{:?}",
        label.at
    );
    assert_eq!(proj.pick(label.at[0], label.at[1], 0.1), Some(id));
    assert_eq!(proj.pick(30.0, -10.0, 0.1), Some(id));
    let snap = proj.snap(
        40.0,
        -10.0,
        0.5,
        SnapOptions {
            grid: false,
            objects: true,
        },
        &[],
    );
    assert_eq!(snap.kind, SnapKind::None);
}

#[test]
fn dimensions_are_saved_and_opened() {
    let mut proj = Project::new();
    circle(&mut proj, p(0.0, 0.0), 5.0);
    dimension_by_clicks(&mut proj, &[p(5.0, 0.1), p(10.0, 3.0)]).unwrap();
    dimension_by_clicks(&mut proj, &[p(20.0, 0.0), p(24.0, 3.0), p(29.0, 9.0)]).unwrap();
    let text = to_text(proj.document());
    assert!(text.contains(&format!("\"format_version\": {FORMAT_VERSION}")));
    assert_eq!(FORMAT_VERSION, 2);
    assert_eq!(&from_text(&text).unwrap(), proj.document());
}

#[test]
fn a_version_1_project_still_opens() {
    let text = to_text(Project::new().document()).replace(
        &format!("\"format_version\": {FORMAT_VERSION}"),
        "\"format_version\": 1",
    );
    assert!(from_text(&text).is_ok());
}
