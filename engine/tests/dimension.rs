//! Dimensions (spec 5.3): placing each kind by clicking, the measured values, picking,
//! transforms, saving, and operations refusing them.

use wham_engine::geometry::{Polyline, Vertex};
use wham_engine::project_file::{FORMAT_VERSION, from_text, to_text};
use wham_engine::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn draft(tool: DrawTool, points: &[Point], picks: &[u32]) -> Draft {
    Draft {
        tool,
        points: points.to_vec(),
        arcs: Vec::new(),
        arc_next: false,
        picks: picks.to_vec(),
    }
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

#[test]
fn aligned_measures_the_distance_and_sits_through_the_third_click() {
    let mut proj = Project::new();
    let d = draft(
        DrawTool::DimAligned,
        &[p(0.0, 0.0), p(30.0, 40.0), p(-8.0, 6.0)],
        &[],
    );
    let id = proj.add_draft("Default", &d).unwrap();
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
    assert_eq!(label.text, "50.00");
    assert!(close(label.angle, 53.13010235415598), "{}", label.angle);
}

#[test]
fn aligned_needs_two_different_points() {
    let mut proj = Project::new();
    let d = draft(
        DrawTool::DimAligned,
        &[p(1.0, 1.0), p(1.0, 1.0), p(5.0, 5.0)],
        &[],
    );
    assert!(proj.add_draft("Default", &d).is_err());
}

#[test]
fn radius_and_diameter_of_a_circle() {
    let mut proj = Project::new();
    let c = proj
        .add_shape(
            "Default",
            Shape::Circle {
                center: p(10.0, 10.0),
                radius: 12.5,
            },
        )
        .unwrap();
    // A click near the circle picks it; a click near the centre doesn't.
    assert_eq!(
        proj.dimension_pick(DrawTool::DimRadius, 0, 22.4, 10.0, 0.5),
        Some(c)
    );
    assert_eq!(
        proj.dimension_pick(DrawTool::DimRadius, 0, 10.0, 10.0, 0.5),
        None
    );
    // The second click is where the value goes, not a pick.
    assert_eq!(
        proj.dimension_pick(DrawTool::DimRadius, 1, 22.4, 10.0, 0.5),
        None
    );

    let click = p(22.5, 10.0);
    let r = proj
        .add_draft(
            "Default",
            &draft(DrawTool::DimRadius, &[click, p(40.0, 20.0)], &[c]),
        )
        .unwrap();
    let d = proj
        .add_draft(
            "Default",
            &draft(DrawTool::DimDiameter, &[click, p(40.0, 20.0)], &[c]),
        )
        .unwrap();
    assert_eq!(dimension(&proj, r).text(), "R12.50");
    assert_eq!(dimension(&proj, d).text(), "Ø25.00");
}

#[test]
fn radius_of_an_arc_segment_in_a_polyline() {
    // The arc test shape: convex arc R37.5 on the right, concave arc R72.5 on top.
    let mut proj = Project::new();
    let v = |x: f64, y: f64, bulge: f64| Vertex { x, y, bulge };
    let id = proj
        .add_shape(
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
    // The right-hand arc bulges out to x = 115.
    assert_eq!(
        proj.dimension_pick(DrawTool::DimRadius, 0, 115.0, 30.0, 0.5),
        Some(id)
    );
    let r = proj
        .add_draft(
            "Default",
            &draft(
                DrawTool::DimRadius,
                &[p(115.0, 30.0), p(130.0, 30.0)],
                &[id],
            ),
        )
        .unwrap();
    assert!(close(dimension(&proj, r).value(), 37.5));
    // The top arc dips to y = 40.
    let top = proj
        .add_draft(
            "Default",
            &draft(DrawTool::DimRadius, &[p(50.0, 40.0), p(50.0, 20.0)], &[id]),
        )
        .unwrap();
    assert!(close(dimension(&proj, top).value(), 72.5));
    // A straight side is not a circle.
    assert_eq!(
        proj.dimension_pick(DrawTool::DimRadius, 0, 50.0, 0.0, 0.5),
        None
    );
}

#[test]
fn angle_takes_the_angle_the_third_click_is_in() {
    let mut proj = Project::new();
    let a = line(&mut proj, p(0.0, 0.0), p(10.0, 0.0));
    let b = line(&mut proj, p(0.0, 0.0), p(10.0, 10.0));
    assert_eq!(
        proj.dimension_pick(DrawTool::DimAngle, 0, 5.0, 0.1, 0.5),
        Some(a)
    );
    assert_eq!(
        proj.dimension_pick(DrawTool::DimAngle, 1, 5.0, 5.1, 0.5),
        Some(b)
    );
    let clicks = [p(5.0, 0.0), p(5.0, 5.0)];
    let angle_at = |proj: &mut Project, at: Point| {
        let mut pts = clicks.to_vec();
        pts.push(at);
        let id = proj
            .add_draft("Default", &draft(DrawTool::DimAngle, &pts, &[a, b]))
            .unwrap();
        dimension(proj, id).value()
    };
    // Between the lines: 45°. Above the slanted line: 135°. The other two are the same.
    assert!(close(angle_at(&mut proj, p(8.0, 3.0)), 45.0));
    assert!(close(angle_at(&mut proj, p(-1.0, 8.0)), 135.0));
    assert!(close(angle_at(&mut proj, p(-8.0, -3.0)), 45.0));
    assert!(close(angle_at(&mut proj, p(1.0, -8.0)), 135.0));
}

#[test]
fn angle_between_a_rectangle_side_and_a_line_that_dont_meet() {
    // The lines are extended to where they would cross.
    let mut proj = Project::new();
    let r = proj
        .add_shape("Default", Shape::rectangle(p(0.0, 0.0), 20.0, 10.0, 0.0))
        .unwrap();
    let l = line(&mut proj, p(30.0, 5.0), p(40.0, 15.0));
    // Bottom side of the rectangle, and the slanted line (crossing at (25, 0)).
    let d = draft(
        DrawTool::DimAngle,
        &[p(10.0, 0.0), p(35.0, 10.0), p(40.0, 2.0)],
        &[r, l],
    );
    let id = proj.add_draft("Default", &d).unwrap();
    let Dimension::Angle { vertex, value, .. } = dimension(&proj, id) else {
        panic!()
    };
    assert!(close(vertex.x, 25.0) && close(vertex.y, 0.0), "{vertex:?}");
    assert!(close(value, 45.0));
    assert_eq!(dimension(&proj, id).text(), "45.00°");
}

#[test]
fn parallel_lines_have_no_angle() {
    let mut proj = Project::new();
    let a = line(&mut proj, p(0.0, 0.0), p(10.0, 0.0));
    let b = line(&mut proj, p(0.0, 5.0), p(10.0, 5.0));
    let d = draft(
        DrawTool::DimAngle,
        &[p(5.0, 0.0), p(5.0, 5.0), p(5.0, 2.0)],
        &[a, b],
    );
    let err = proj.add_draft("Default", &d).unwrap_err();
    assert!(err.contains("parallel"), "{err}");
}

#[test]
fn moving_and_mirroring_keep_the_value_and_scaling_scales_lengths() {
    let mut proj = Project::new();
    let al = proj
        .add_draft(
            "Default",
            &draft(
                DrawTool::DimAligned,
                &[p(0.0, 0.0), p(10.0, 0.0), p(5.0, 5.0)],
                &[],
            ),
        )
        .unwrap();
    let a = line(&mut proj, p(0.0, 0.0), p(10.0, 0.0));
    let b = line(&mut proj, p(0.0, 0.0), p(10.0, 10.0));
    let an = proj
        .add_draft(
            "Default",
            &draft(
                DrawTool::DimAngle,
                &[p(5.0, 0.0), p(5.0, 5.0), p(8.0, 3.0)],
                &[a, b],
            ),
        )
        .unwrap();
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
    let c = proj
        .add_shape(
            "Default",
            Shape::Circle {
                center: p(0.0, 0.0),
                radius: 5.0,
            },
        )
        .unwrap();
    let d = proj
        .add_draft(
            "Default",
            &draft(DrawTool::DimDiameter, &[p(5.0, 0.0), p(10.0, 0.0)], &[c]),
        )
        .unwrap();
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
    let id = proj
        .add_draft(
            "Default",
            &draft(
                DrawTool::DimAligned,
                &[p(0.0, 0.0), p(40.0, 0.0), p(20.0, -10.0)],
                &[],
            ),
        )
        .unwrap();
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
    let c = proj
        .add_shape(
            "Default",
            Shape::Circle {
                center: p(0.0, 0.0),
                radius: 5.0,
            },
        )
        .unwrap();
    proj.add_draft(
        "Default",
        &draft(DrawTool::DimRadius, &[p(5.0, 0.0), p(10.0, 3.0)], &[c]),
    )
    .unwrap();
    proj.add_draft(
        "Default",
        &draft(
            DrawTool::DimAligned,
            &[p(0.0, 0.0), p(4.0, 3.0), p(9.0, 9.0)],
            &[],
        ),
    )
    .unwrap();
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
