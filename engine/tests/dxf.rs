//! DXF reading tests. Each test builds a tiny DXF from code/value pairs, so the
//! expected result is easy to check by hand.

use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2};

use wham_engine::*;

const EPS: f64 = 1e-9;

/// One entity as DXF text.
fn ent(kind: &str, pairs: &[(i32, &str)]) -> String {
    let mut s = format!("0\n{kind}\n");
    for (c, v) in pairs {
        s += &format!("{c}\n{v}\n");
    }
    s
}

/// A DXF file with an optional $INSUNITS header, optional BLOCKS text, and ENTITIES text.
fn dxf(insunits: Option<i32>, blocks: &str, entities: &str) -> String {
    let mut s = String::new();
    if let Some(u) = insunits {
        s += &format!("0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n{u}\n0\nENDSEC\n");
    }
    if !blocks.is_empty() {
        s += &format!("0\nSECTION\n2\nBLOCKS\n{blocks}0\nENDSEC\n");
    }
    s += &format!("0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n");
    s
}

fn read(text: &str) -> DxfDrawing {
    read_dxf(text).expect("valid DXF")
}

#[test]
fn reads_line_circle_and_arc_with_layers() {
    let text = dxf(
        None,
        "",
        &(ent(
            "LINE",
            &[(8, "CUT"), (10, "0"), (20, "0"), (11, "10"), (21, "5")],
        ) + &ent("CIRCLE", &[(8, "HOLES"), (10, "5"), (20, "6"), (40, "2.5")])
            + &ent(
                "ARC",
                &[(10, "1"), (20, "2"), (40, "3"), (50, "270"), (51, "90")],
            )),
    );
    let d = read(&text);
    assert!(d.skipped.is_empty());
    assert_eq!(
        d.shapes,
        vec![
            (
                "CUT".to_string(),
                Shape::Line {
                    start: Point::new(0.0, 0.0),
                    end: Point::new(10.0, 5.0)
                }
            ),
            (
                "HOLES".to_string(),
                Shape::Circle {
                    center: Point::new(5.0, 6.0),
                    radius: 2.5
                }
            ),
            (
                // No layer given: DXF default layer "0". An arc that crosses 0 deg runs 270 -> 450.
                "0".to_string(),
                Shape::Arc {
                    center: Point::new(1.0, 2.0),
                    radius: 3.0,
                    start_angle: 270.0,
                    end_angle: 450.0
                }
            ),
        ]
    );
}

#[test]
fn reads_lwpolyline_with_bulge_and_closed_flag() {
    let text = dxf(
        None,
        "",
        &ent(
            "LWPOLYLINE",
            &[
                (90, "3"),
                (70, "1"),
                (10, "0"),
                (20, "0"),
                (10, "10"),
                (20, "0"),
                (42, "1"),
                (10, "10"),
                (20, "10"),
            ],
        ),
    );
    let d = read(&text);
    let Shape::Polyline(pl) = &d.shapes[0].1 else {
        panic!("expected polyline")
    };
    assert!(pl.closed);
    assert_eq!(
        pl.vertices,
        vec![
            Vertex::line(0.0, 0.0),
            Vertex {
                x: 10.0,
                y: 0.0,
                bulge: 1.0
            },
            Vertex::line(10.0, 10.0),
        ]
    );
}

#[test]
fn reads_old_style_polyline_and_skips_3d_polylines() {
    let poly2d = ent("POLYLINE", &[(8, "A"), (66, "1"), (70, "0")])
        + &ent("VERTEX", &[(8, "A"), (10, "0"), (20, "0"), (42, "-0.5")])
        + &ent("VERTEX", &[(8, "A"), (10, "5"), (20, "0")])
        + &ent("VERTEX", &[(8, "A"), (10, "5"), (20, "5")])
        + &ent("SEQEND", &[(8, "A")]);
    let poly3d = ent("POLYLINE", &[(70, "8")])
        + &ent("VERTEX", &[(10, "0"), (20, "0"), (30, "1")])
        + &ent("VERTEX", &[(10, "1"), (20, "1"), (30, "2")])
        + &ent("SEQEND", &[]);
    let d = read(&dxf(None, "", &(poly2d + &poly3d)));
    assert_eq!(d.shapes.len(), 1);
    let Shape::Polyline(pl) = &d.shapes[0].1 else {
        panic!()
    };
    assert!(!pl.closed);
    assert_eq!(pl.vertices.len(), 3);
    assert_eq!(pl.vertices[0].bulge, -0.5);
    assert_eq!(d.skipped.get("POLYLINE (3D or mesh)"), Some(&1));
}

#[test]
fn points_are_grouped_into_one_point_list_per_layer() {
    let text = dxf(
        None,
        "",
        &(ent("POINT", &[(8, "DRILL"), (10, "1"), (20, "1")])
            + &ent("POINT", &[(8, "OTHER"), (10, "9"), (20, "9")])
            + &ent("POINT", &[(8, "DRILL"), (10, "2"), (20, "2")])),
    );
    let d = read(&text);
    assert_eq!(
        d.shapes,
        vec![
            (
                "DRILL".to_string(),
                Shape::Points {
                    points: vec![Point::new(1.0, 1.0), Point::new(2.0, 2.0)]
                }
            ),
            (
                "OTHER".to_string(),
                Shape::Points {
                    points: vec![Point::new(9.0, 9.0)]
                }
            ),
        ]
    );
}

#[test]
fn arc_with_downward_extrusion_is_mirrored() {
    // Some CAD programs write arcs with extrusion (0,0,-1): their X is mirrored.
    let text = dxf(
        None,
        "",
        &ent(
            "ARC",
            &[
                (10, "10"),
                (20, "0"),
                (40, "5"),
                (50, "0"),
                (51, "90"),
                (210, "0"),
                (220, "0"),
                (230, "-1"),
            ],
        ),
    );
    let Shape::Arc {
        center,
        start_angle,
        end_angle,
        ..
    } = read(&text).shapes[0].1.clone()
    else {
        panic!()
    };
    assert_eq!(center, Point::new(-10.0, 0.0));
    assert!((start_angle - 90.0).abs() < EPS);
    assert!((end_angle - 180.0).abs() < EPS);
}

#[test]
fn tilted_entities_are_skipped() {
    let text = dxf(
        None,
        "",
        &ent(
            "CIRCLE",
            &[
                (10, "0"),
                (20, "0"),
                (40, "1"),
                (210, "1"),
                (220, "0"),
                (230, "0"),
            ],
        ),
    );
    let d = read(&text);
    assert!(d.shapes.is_empty());
    assert_eq!(d.skipped.get("CIRCLE (not flat in XY)"), Some(&1));
}

#[test]
fn block_insert_is_placed_rotated_scaled_and_takes_the_insert_layer() {
    let blocks = ent("BLOCK", &[(2, "PEG"), (10, "0"), (20, "0")])
        + &ent(
            "LINE",
            &[(8, "0"), (10, "0"), (20, "0"), (11, "10"), (21, "0")],
        )
        + &ent("CIRCLE", &[(8, "KEEP"), (10, "10"), (20, "0"), (40, "1")])
        + &ent("ENDBLK", &[]);
    let entities = ent(
        "INSERT",
        &[
            (8, "PARTS"),
            (2, "PEG"),
            (10, "100"),
            (20, "50"),
            (41, "2"),
            (42, "2"),
            (50, "90"),
        ],
    );
    let d = read(&dxf(None, &blocks, &entities));
    assert_eq!(d.shapes.len(), 2);
    let (layer, Shape::Line { start, end }) = &d.shapes[0] else {
        panic!()
    };
    assert_eq!(layer, "PARTS");
    assert!(start.distance_to(Point::new(100.0, 50.0)) < EPS);
    assert!(end.distance_to(Point::new(100.0, 70.0)) < EPS, "{end:?}");
    let (layer, Shape::Circle { center, radius }) = &d.shapes[1] else {
        panic!()
    };
    assert_eq!(layer, "KEEP");
    assert!(center.distance_to(Point::new(100.0, 70.0)) < EPS);
    assert!((radius - 2.0).abs() < EPS);
}

#[test]
fn unevenly_scaled_block_keeps_lines_but_skips_circles() {
    let blocks = ent("BLOCK", &[(2, "B"), (10, "0"), (20, "0")])
        + &ent("LINE", &[(10, "0"), (20, "0"), (11, "1"), (21, "1")])
        + &ent("CIRCLE", &[(10, "0"), (20, "0"), (40, "1")])
        + &ent("ENDBLK", &[]);
    let entities = ent(
        "INSERT",
        &[(2, "B"), (10, "0"), (20, "0"), (41, "2"), (42, "1")],
    );
    let d = read(&dxf(None, &blocks, &entities));
    assert_eq!(d.shapes.len(), 1);
    assert_eq!(d.skipped.get("CIRCLE (unevenly scaled block)"), Some(&1));
}

#[test]
fn missing_blocks_unsupported_entities_and_paper_space_are_counted() {
    let text = dxf(
        None,
        "",
        &(ent("TEXT", &[(10, "0"), (20, "0"), (1, "hello")])
            + &ent("TEXT", &[(10, "0"), (20, "0"), (1, "again")])
            + &ent("HATCH", &[])
            + &ent("INSERT", &[(2, "NOPE")])
            + &ent(
                "LINE",
                &[(67, "1"), (10, "0"), (20, "0"), (11, "1"), (21, "1")],
            )
            + &ent("LINE", &[(10, "0"), (20, "oops"), (11, "1"), (21, "1")])),
    );
    let d = read(&text);
    assert!(d.shapes.is_empty());
    assert_eq!(d.skipped.get("TEXT"), Some(&2));
    assert_eq!(d.skipped.get("HATCH"), Some(&1));
    assert_eq!(d.skipped.get("INSERT (missing block)"), Some(&1));
    assert_eq!(d.skipped.get("LINE (paper space)"), Some(&1));
    assert_eq!(d.skipped.get("LINE (unreadable)"), Some(&1));
}

#[test]
fn reads_units_from_the_header() {
    let inch = read(&dxf(Some(1), "", ""));
    assert_eq!(inch.units.name, "inches");
    assert_eq!(inch.units.mm_per_unit, Some(25.4));
    let mm = read(&dxf(Some(4), "", ""));
    assert_eq!(mm.units.mm_per_unit, Some(1.0));
    let none = read(&dxf(None, "", ""));
    assert_eq!(none.units.code, 0);
    assert_eq!(none.units.mm_per_unit, None);
}

#[test]
fn bad_files_give_errors_not_crashes() {
    assert_eq!(read_dxf(""), Err(DxfError::Empty));
    assert_eq!(
        read_dxf("AutoCAD Binary DXF\r\n\u{1a}\0..."),
        Err(DxfError::Binary)
    );
    assert!(matches!(
        read_dxf("hello\nworld\n"),
        Err(DxfError::Malformed { line: 1, .. })
    ));
    assert!(matches!(
        read_dxf("0\nSECTION\n2"),
        Err(DxfError::Malformed { .. })
    ));
    // Windows line endings and a byte-order mark are fine.
    let crlf = dxf(None, "", &ent("POINT", &[(10, "1"), (20, "2")])).replace('\n', "\r\n");
    assert_eq!(read(&format!("\u{feff}{crlf}")).shapes.len(), 1);
}

#[test]
fn rational_spline_quarter_circle_is_drawn_on_the_circle() {
    let w = FRAC_1_SQRT_2.to_string();
    let text = dxf(
        None,
        "",
        &ent(
            "SPLINE",
            &[
                (70, "12"),
                (71, "2"),
                (72, "6"),
                (73, "3"),
                (40, "0"),
                (40, "0"),
                (40, "0"),
                (40, "1"),
                (40, "1"),
                (40, "1"),
                (41, "1"),
                (41, &w),
                (41, "1"),
                (10, "1"),
                (20, "0"),
                (10, "1"),
                (20, "1"),
                (10, "0"),
                (20, "1"),
            ],
        ),
    );
    let d = read(&text);
    let Shape::Spline(s) = &d.shapes[0].1 else {
        panic!()
    };
    assert_eq!(s.degree, 2);
    assert_eq!(s.weights.len(), 3);
    let shape = display::display_shape(1, &d.shapes[0].1);
    assert_eq!(shape.points.first(), Some(&[1.0, 0.0]));
    let last = shape.points.last().unwrap();
    assert!((last[0]).abs() < EPS && (last[1] - 1.0).abs() < EPS);
    for p in &shape.points {
        assert!(
            (p[0].hypot(p[1]) - 1.0).abs() < 1e-9,
            "{p:?} is off the circle"
        );
    }
}

#[test]
fn spline_with_only_fit_points_is_skipped() {
    let text = dxf(
        None,
        "",
        &ent(
            "SPLINE",
            &[(71, "3"), (11, "0"), (21, "0"), (11, "1"), (21, "1")],
        ),
    );
    let d = read(&text);
    assert!(d.shapes.is_empty());
    assert_eq!(d.skipped.get("SPLINE (fit points only)"), Some(&1));
}

#[test]
fn ellipse_points_lie_on_the_ellipse() {
    let text = dxf(
        None,
        "",
        &ent(
            "ELLIPSE",
            &[(10, "0"), (20, "0"), (11, "10"), (21, "0"), (40, "0.5")],
        ),
    );
    let d = read(&text);
    let shape = display::display_shape(1, &d.shapes[0].1);
    assert!(shape.points.len() > 32);
    for p in &shape.points {
        let v = (p[0] / 10.0).powi(2) + (p[1] / 5.0).powi(2);
        assert!((v - 1.0).abs() < 1e-9, "{p:?}");
    }
}

#[test]
fn partial_ellipse_with_downward_extrusion_runs_the_other_way() {
    // Params 0..pi/2 with extrusion -1: from (10,0) clockwise to (0,-5).
    let text = dxf(
        None,
        "",
        &ent(
            "ELLIPSE",
            &[
                (10, "0"),
                (20, "0"),
                (11, "10"),
                (21, "0"),
                (40, "0.5"),
                (41, "0"),
                (42, &FRAC_PI_2.to_string()),
                (230, "-1"),
            ],
        ),
    );
    let shape = display::display_shape(1, &read(&text).shapes[0].1);
    let (first, last) = (shape.points[0], *shape.points.last().unwrap());
    assert!(
        (first[0]).abs() < 1e-9 && (first[1] + 5.0).abs() < 1e-9,
        "{first:?}"
    );
    assert!(
        (last[0] - 10.0).abs() < 1e-9 && last[1].abs() < 1e-9,
        "{last:?}"
    );
}
