//! Drawing tests: building shapes from clicks, snapping, validation, and undo.

use wham_engine::draw::{finish, preview, tangent_bulge, validate};
use wham_engine::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn draft(tool: DrawTool, points: &[(f64, f64)]) -> Draft {
    Draft {
        tool,
        points: points.iter().map(|&(x, y)| p(x, y)).collect(),
        arcs: Vec::new(),
        arc_next: false,
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn assert_point(a: Point, x: f64, y: f64) {
    assert!(
        close(a.x, x) && close(a.y, y),
        "expected ({x}, {y}), got {a:?}"
    );
}

const ALL_SNAPS: SnapOptions = SnapOptions {
    grid: true,
    objects: true,
};

// --- Building shapes -------------------------------------------------------------------------

#[test]
fn line_and_circle_from_two_clicks() {
    let line = finish(&draft(DrawTool::Line, &[(1.0, 2.0), (3.0, 4.0)])).unwrap();
    assert_eq!(
        line,
        Shape::Line {
            start: p(1.0, 2.0),
            end: p(3.0, 4.0)
        }
    );
    let circle = finish(&draft(DrawTool::Circle, &[(10.0, 10.0), (13.0, 14.0)])).unwrap();
    assert_eq!(
        circle,
        Shape::Circle {
            center: p(10.0, 10.0),
            radius: 5.0
        }
    );
}

#[test]
fn wrong_number_of_clicks_is_refused() {
    assert!(finish(&draft(DrawTool::Line, &[(0.0, 0.0)])).is_err());
    assert!(finish(&draft(DrawTool::Arc, &[(0.0, 0.0), (1.0, 0.0)])).is_err());
}

#[test]
fn rectangle_tool_makes_a_poly_rectangle_from_its_lower_left_corner() {
    let r = finish(&draft(DrawTool::Rectangle, &[(30.0, 20.0), (10.0, 5.0)])).unwrap();
    assert_eq!(
        r,
        Shape::Rectangle {
            lower_left: p(10.0, 5.0),
            width: 20.0,
            height: 15.0,
            rotation: 0.0
        }
    );
    assert_eq!(r.type_name(), "PolyRectangle");
    // A rectangle with no width is refused.
    assert!(finish(&draft(DrawTool::Rectangle, &[(1.0, 1.0), (1.0, 9.0)])).is_err());
}

#[test]
fn rectangle_snaps_to_corners_side_mids_and_centre() {
    let mut pr = Project::new();
    pr.add_draft(
        "Default",
        &draft(DrawTool::Rectangle, &[(0.0, 0.0), (20.0, 10.0)]),
    )
    .unwrap();
    let s = pr.snap(19.9, 9.9, 0.5, ALL_SNAPS, &[]);
    assert_eq!((s.kind, s.point), (SnapKind::End, p(20.0, 10.0)));
    let s = pr.snap(10.1, 0.1, 0.5, ALL_SNAPS, &[]);
    assert_eq!((s.kind, s.point), (SnapKind::Mid, p(10.0, 0.0)));
    let s = pr.snap(10.1, 5.1, 0.5, ALL_SNAPS, &[]);
    assert_eq!((s.kind, s.point), (SnapKind::Centre, p(10.0, 5.0)));
}

#[test]
fn arc_centre_slides_onto_the_bisector_and_runs_ccw_short() {
    // Start (10,0), end (0,10); centre clicked near the origin but off the bisector.
    let arc = finish(&draft(
        DrawTool::Arc,
        &[(10.0, 0.0), (0.0, 10.0), (0.3, -0.3)],
    ))
    .unwrap();
    let Shape::Arc {
        center,
        radius,
        start_angle,
        end_angle,
    } = arc
    else {
        panic!("not an arc: {arc:?}");
    };
    assert_point(center, 0.0, 0.0);
    assert!(close(radius, 10.0));
    assert!(close(start_angle, 0.0));
    assert!(close(end_angle, 90.0));
}

#[test]
fn arc_centre_on_the_other_side_gives_the_long_way_round() {
    // Same ends, centre clicked at (10,10): CCW from (10,0) to (0,10) round (10,10) is 270°.
    let arc = finish(&draft(
        DrawTool::Arc,
        &[(10.0, 0.0), (0.0, 10.0), (10.0, 10.0)],
    ))
    .unwrap();
    let Shape::Arc {
        center,
        start_angle,
        end_angle,
        ..
    } = arc
    else {
        panic!("not an arc");
    };
    assert_point(center, 10.0, 10.0);
    assert!(close(start_angle, 270.0));
    assert!(close(end_angle, 540.0));
}

#[test]
fn arc_with_same_start_and_end_is_refused() {
    assert!(finish(&draft(DrawTool::Arc, &[(1.0, 1.0), (1.0, 1.0), (5.0, 5.0)])).is_err());
}

#[test]
fn arc_preview_is_a_line_until_the_centre_is_given() {
    let d = draft(DrawTool::Arc, &[(0.0, 0.0)]);
    assert!(matches!(
        preview(&d, Some(p(5.0, 0.0))),
        Some(Shape::Line { .. })
    ));
    let d = draft(DrawTool::Arc, &[(0.0, 0.0), (10.0, 0.0)]);
    assert!(matches!(
        preview(&d, Some(p(5.0, 3.0))),
        Some(Shape::Arc { .. })
    ));
}

#[test]
fn tangent_bulge_makes_a_semicircle_when_turning_back() {
    // Heading +X at (0,0), going to (0,10): a CCW half circle, bulge 1.
    assert!(close(
        tangent_bulge(p(1.0, 0.0), p(0.0, 0.0), p(0.0, 10.0)),
        1.0
    ));
    // Heading +X, going to (0,-10): clockwise half circle, bulge -1.
    assert!(close(
        tangent_bulge(p(1.0, 0.0), p(0.0, 0.0), p(0.0, -10.0)),
        -1.0
    ));
    // Straight ahead: no bulge.
    assert!(close(
        tangent_bulge(p(1.0, 0.0), p(0.0, 0.0), p(5.0, 0.0)),
        0.0
    ));
}

#[test]
fn polyline_tangent_arc_follows_previous_segment() {
    // Straight to (10,0), then a tangent arc up to (10,10): a CCW half circle (bulge 1),
    // then a tangent arc back to (0,10) continues heading -X: straight on, bulge 0.
    let mut d = draft(
        DrawTool::Polyline,
        &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)],
    );
    d.arcs = vec![false, true, true];
    let Shape::Polyline(pl) = finish(&d).unwrap() else {
        panic!("not a polyline")
    };
    let bulges: Vec<f64> = pl.vertices.iter().map(|v| v.bulge).collect();
    assert!(close(bulges[0], 0.0));
    assert!(close(bulges[1], 1.0));
    assert!(close(bulges[2], 0.0));
    assert!(!pl.closed);
}

#[test]
fn first_polyline_segment_is_always_straight() {
    let mut d = draft(DrawTool::Polyline, &[(0.0, 0.0), (10.0, 5.0)]);
    d.arcs = vec![true];
    let Shape::Polyline(pl) = finish(&d).unwrap() else {
        panic!()
    };
    assert_eq!(pl.vertices[0].bulge, 0.0);
}

#[test]
fn polyline_back_to_first_point_closes() {
    let d = draft(
        DrawTool::Polyline,
        &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 0.0)],
    );
    let Shape::Polyline(pl) = finish(&d).unwrap() else {
        panic!()
    };
    assert!(pl.closed);
    assert_eq!(pl.vertices.len(), 3);
}

#[test]
fn polyline_drops_repeated_points_from_double_click() {
    let d = draft(DrawTool::Polyline, &[(0.0, 0.0), (10.0, 0.0), (10.0, 0.0)]);
    let Shape::Polyline(pl) = finish(&d).unwrap() else {
        panic!()
    };
    assert_eq!(pl.vertices.len(), 2);
    assert!(finish(&draft(DrawTool::Polyline, &[(1.0, 1.0), (1.0, 1.0)])).is_err());
}

#[test]
fn polyline_preview_uses_arc_next_for_the_cursor_segment() {
    let mut d = draft(DrawTool::Polyline, &[(0.0, 0.0), (10.0, 0.0)]);
    d.arc_next = true;
    let Some(Shape::Polyline(pl)) = preview(&d, Some(p(10.0, 10.0))) else {
        panic!()
    };
    assert!(close(pl.vertices[1].bulge, 1.0));
}

#[test]
fn points_tool_makes_one_point_list() {
    let d = draft(DrawTool::Points, &[(1.0, 1.0), (2.0, 2.0)]);
    assert_eq!(
        finish(&d).unwrap(),
        Shape::Points {
            points: vec![p(1.0, 1.0), p(2.0, 2.0)]
        }
    );
    assert!(finish(&draft(DrawTool::Points, &[])).is_err());
}

// --- Validation ------------------------------------------------------------------------------

#[test]
fn validation_rejects_bad_shapes() {
    let bad = [
        Shape::Circle {
            center: p(0.0, 0.0),
            radius: 0.0,
        },
        Shape::Circle {
            center: p(f64::NAN, 0.0),
            radius: 1.0,
        },
        Shape::Line {
            start: p(1.0, 1.0),
            end: p(1.0, 1.0),
        },
        Shape::Arc {
            center: p(0.0, 0.0),
            radius: -1.0,
            start_angle: 0.0,
            end_angle: 90.0,
        },
        Shape::Points { points: vec![] },
    ];
    for s in bad {
        assert!(validate(&s).is_err(), "{s:?} should be refused");
    }
}

// --- Snapping --------------------------------------------------------------------------------

fn project_with_line() -> Project {
    let mut pr = Project::new();
    pr.add_shape(
        "Default",
        Shape::Line {
            start: p(0.0, 0.0),
            end: p(33.0, 0.0),
        },
    )
    .unwrap();
    pr
}

#[test]
fn snaps_to_grid_when_nothing_is_near() {
    let pr = project_with_line();
    let s = pr.snap(12.4, 27.6, 0.5, ALL_SNAPS, &[]);
    assert_eq!(s.kind, SnapKind::Grid);
    assert_point(s.point, 10.0, 30.0);
}

#[test]
fn object_snaps_win_over_grid() {
    let pr = project_with_line();
    let end = pr.snap(32.8, 0.1, 0.5, ALL_SNAPS, &[]);
    assert_eq!(end.kind, SnapKind::End);
    assert_point(end.point, 33.0, 0.0);
    let mid = pr.snap(16.4, 0.2, 0.5, ALL_SNAPS, &[]);
    assert_eq!(mid.kind, SnapKind::Mid);
    assert_point(mid.point, 16.5, 0.0);
}

#[test]
fn snaps_to_centres_of_circles_arcs_and_polyline_arcs() {
    let mut pr = Project::new();
    pr.add_shape(
        "Default",
        Shape::Circle {
            center: p(50.0, 50.0),
            radius: 5.0,
        },
    )
    .unwrap();
    // Half circle from (100,0) to (110,0), CCW (bulge 1): centre (105,0), mid (105,-5).
    pr.add_shape(
        "Default",
        Shape::Polyline(Polyline {
            vertices: vec![
                Vertex {
                    x: 100.0,
                    y: 0.0,
                    bulge: 1.0,
                },
                Vertex::line(110.0, 0.0),
            ],
            closed: false,
        }),
    )
    .unwrap();
    let c = pr.snap(50.2, 49.9, 0.5, ALL_SNAPS, &[]);
    assert_eq!((c.kind, c.point), (SnapKind::Centre, p(50.0, 50.0)));
    let c = pr.snap(105.1, 0.1, 0.5, ALL_SNAPS, &[]);
    assert_eq!(c.kind, SnapKind::Centre);
    assert_point(c.point, 105.0, 0.0);
    let m = pr.snap(105.1, -4.9, 0.5, ALL_SNAPS, &[]);
    assert_eq!(m.kind, SnapKind::Mid);
    assert_point(m.point, 105.0, -5.0);
}

#[test]
fn arc_ends_and_mid_snap() {
    let mut pr = Project::new();
    pr.add_shape(
        "Default",
        Shape::Arc {
            center: p(0.0, 0.0),
            radius: 10.0,
            start_angle: 0.0,
            end_angle: 90.0,
        },
    )
    .unwrap();
    let s = pr.snap(0.1, 9.9, 0.5, ALL_SNAPS, &[]);
    assert_eq!(s.kind, SnapKind::End);
    assert_point(s.point, 0.0, 10.0);
    let r = 10.0 / 2f64.sqrt();
    let s = pr.snap(r, r, 0.5, ALL_SNAPS, &[]);
    assert_eq!(s.kind, SnapKind::Mid);
    assert_point(s.point, r, r);
}

#[test]
fn snaps_to_points_of_the_shape_being_drawn() {
    let pr = Project::new();
    let s = pr.snap(5.1, 5.1, 0.5, ALL_SNAPS, &[p(5.05, 5.05)]);
    assert_eq!((s.kind, s.point), (SnapKind::End, p(5.05, 5.05)));
}

#[test]
fn snaps_can_be_switched_off() {
    let pr = project_with_line();
    let off = SnapOptions {
        grid: false,
        objects: false,
    };
    let s = pr.snap(32.8, 0.1, 0.5, off, &[]);
    assert_eq!((s.kind, s.point), (SnapKind::None, p(32.8, 0.1)));
    let grid_only = SnapOptions {
        grid: true,
        objects: false,
    };
    let s = pr.snap(32.8, 0.1, 0.5, grid_only, &[]);
    assert_eq!(s.kind, SnapKind::Grid);
    assert_point(s.point, 30.0, 0.0);
}

#[test]
fn grid_snap_follows_grid_spacing_and_never_gives_minus_zero() {
    let mut pr = Project::new();
    pr.set_grid_spacing(2.5).unwrap();
    let s = pr.snap(6.4, -0.4, 0.0, ALL_SNAPS, &[]);
    assert_point(s.point, 7.5, 0.0);
    assert!(s.point.y.is_sign_positive());
}

// --- Project changes and undo ----------------------------------------------------------------

#[test]
fn add_draft_goes_on_the_layer_and_undoes() {
    let mut pr = Project::new();
    let id = pr
        .add_draft("CUT", &draft(DrawTool::Line, &[(0.0, 0.0), (5.0, 0.0)]))
        .unwrap();
    let cut = pr
        .document()
        .layers
        .iter()
        .find(|l| l.name == "CUT")
        .unwrap();
    assert_eq!(cut.objects[0].id, id);
    assert_eq!(pr.display().shapes.len(), 1);
    assert!(pr.undo());
    assert_eq!(pr.document().objects().count(), 0);
    assert!(pr.redo());
    assert_eq!(pr.document().objects().count(), 1);
}

#[test]
fn invalid_draft_changes_nothing() {
    let mut pr = Project::new();
    assert!(
        pr.add_draft(
            "Default",
            &draft(DrawTool::Circle, &[(1.0, 1.0), (1.0, 1.0)])
        )
        .is_err()
    );
    assert!(!pr.can_undo());
}

#[test]
fn set_shape_edits_and_undoes() {
    let mut pr = project_with_line();
    let id = pr.document().objects().next().unwrap().id;
    let new = Shape::Line {
        start: p(1.0, 1.0),
        end: p(2.0, 2.0),
    };
    pr.set_shape(id, new.clone()).unwrap();
    assert_eq!(pr.document().objects().next().unwrap().shape, new);
    assert!(
        pr.set_shape(
            id,
            Shape::Circle {
                center: p(0.0, 0.0),
                radius: -2.0
            }
        )
        .is_err()
    );
    assert!(pr.set_shape(999, new).is_err());
    pr.undo();
    assert!(matches!(
        pr.document().objects().next().unwrap().shape,
        Shape::Line { end, .. } if end == p(33.0, 0.0)
    ));
}

#[test]
fn grid_spacing_is_checked_and_undoable() {
    let mut pr = Project::new();
    assert_eq!(pr.document().grid_spacing, DEFAULT_GRID_SPACING);
    assert!(pr.set_grid_spacing(0.0).is_err());
    assert!(pr.set_grid_spacing(f64::NAN).is_err());
    pr.set_grid_spacing(5.0).unwrap();
    assert_eq!(pr.document().grid_spacing, 5.0);
    pr.undo();
    assert_eq!(pr.document().grid_spacing, DEFAULT_GRID_SPACING);
}

#[test]
fn older_documents_without_grid_spacing_load_with_the_default() {
    let json = r#"{"name":"Old","layers":[],"parts":[]}"#;
    let doc: Document = serde_json::from_str(json).unwrap();
    assert_eq!(doc.grid_spacing, DEFAULT_GRID_SPACING);
}

#[test]
fn preview_returns_display_points() {
    let pr = Project::new();
    let d = draft(DrawTool::Circle, &[(0.0, 0.0)]);
    let shape = pr.preview(&d, Some(p(10.0, 0.0))).unwrap();
    assert!(shape.closed);
    assert!(shape.points.len() > 10);
    assert!(pr.preview(&draft(DrawTool::Line, &[]), None).is_none());
}
