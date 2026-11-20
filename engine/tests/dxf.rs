//! DXF reading tests. Each test builds a tiny DXF from code/value pairs, so the
//! expected result is easy to check by hand.

use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2, TAU};

use wham_engine::curve::Curve;
use wham_engine::geometry::{bulge_arc, on_circle};

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
    read_dxf(text, &DxfOptions::default()).expect("valid DXF")
}

/// Points along a polyline: every vertex and 20 points along each segment (arcs followed).
fn along(pl: &Polyline) -> Vec<Point> {
    let n = pl.vertices.len();
    let segs = if pl.closed { n } else { n - 1 };
    let mut out = Vec::new();
    for i in 0..segs {
        let (a, b) = (pl.vertices[i], pl.vertices[(i + 1) % n]);
        for k in 0..20 {
            let f = k as f64 / 20.0;
            out.push(match bulge_arc(a.point(), b.point(), a.bulge) {
                Some(arc) => on_circle(arc.center, arc.radius, arc.start + arc.sweep * f),
                None => Point::new(a.x + (b.x - a.x) * f, a.y + (b.y - a.y) * f),
            });
        }
    }
    out.push(pl.vertices[if pl.closed { 0 } else { n - 1 }].point());
    out
}

/// Distance from `p` to the segment from `a` to `b` with `bulge` (straight when 0).
fn segment_distance(p: Point, a: Point, b: Point, bulge: f64) -> f64 {
    if let Some(arc) = bulge_arc(a, b, bulge) {
        let ang = (p.y - arc.center.y).atan2(p.x - arc.center.x);
        let rel = (ang - arc.start).rem_euclid(TAU);
        let along = if arc.sweep >= 0.0 { rel } else { TAU - rel };
        if rel == 0.0 || along <= arc.sweep.abs() {
            return (p.distance_to(arc.center) - arc.radius).abs();
        }
        return p.distance_to(a).min(p.distance_to(b));
    }
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let f = if len2 == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    };
    p.distance_to(Point::new(a.x + dx * f, a.y + dy * f))
}

/// Checks the polyline stays within `tol` of the curve, both ways: points along the polyline
/// against a fine sampling of the curve (joined by lines), and points of the curve against the
/// polyline's true arcs and lines.
fn assert_within(pl: &Polyline, curve: &Curve, tol: f64) {
    let (u0, u1) = match curve {
        Curve::Ellipse {
            start_param,
            end_param,
            ..
        } => (*start_param, *end_param),
        Curve::Spline(s) => (s.knots[s.degree], s.knots[s.control_points.len()]),
    };
    let fine: Vec<Point> = (0..=20000)
        .map(|i| curve.point(u0 + (u1 - u0) * i as f64 / 20000.0))
        .collect();
    // The fine sampling's own error (chords of the curve) is far below 1e-5 here.
    let slack = 1e-5;
    for p in along(pl) {
        let d = fine
            .windows(2)
            .map(|w| segment_distance(p, w[0], w[1], 0.0))
            .fold(f64::INFINITY, f64::min);
        assert!(
            d <= tol + slack,
            "polyline point {p:?} is {d} from the curve"
        );
    }
    let n = pl.vertices.len();
    let segs = if pl.closed { n } else { n - 1 };
    for p in &fine {
        let d = (0..segs)
            .map(|i| {
                let (a, b) = (pl.vertices[i], pl.vertices[(i + 1) % n]);
                segment_distance(*p, a.point(), b.point(), a.bulge)
            })
            .fold(f64::INFINITY, f64::min);
        assert!(
            d <= tol + slack,
            "curve point {p:?} is {d} from the polyline"
        );
    }
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
    assert_eq!(read_dxf("", &DxfOptions::default()), Err(DxfError::Empty));
    assert_eq!(
        read_dxf("AutoCAD Binary DXF\r\n\u{1a}\0...", &DxfOptions::default()),
        Err(DxfError::Binary)
    );
    assert!(matches!(
        read_dxf("hello\nworld\n", &DxfOptions::default()),
        Err(DxfError::Malformed { line: 1, .. })
    ));
    assert!(matches!(
        read_dxf("0\nSECTION\n2", &DxfOptions::default()),
        Err(DxfError::Malformed { .. })
    ));
    // Windows line endings and a byte-order mark are fine.
    let crlf = dxf(None, "", &ent("POINT", &[(10, "1"), (20, "2")])).replace('\n', "\r\n");
    assert_eq!(read(&format!("\u{feff}{crlf}")).shapes.len(), 1);
}

#[test]
fn rational_spline_quarter_circle_becomes_arcs_on_the_circle() {
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
    assert_eq!(d.curves, 1);
    let Shape::Polyline(pl) = &d.shapes[0].1 else {
        panic!("{:?}", d.shapes[0].1)
    };
    assert!(!pl.closed);
    let (first, last) = (pl.vertices[0], *pl.vertices.last().unwrap());
    assert!(
        (first.x - 1.0).abs() < EPS && first.y.abs() < EPS,
        "{first:?}"
    );
    assert!(last.x.abs() < EPS && (last.y - 1.0).abs() < EPS, "{last:?}");
    // A circle is fitted exactly by arcs: a biarc (at most 2 arcs), every point on the circle.
    assert!(pl.vertices.len() <= 3, "{pl:?}");
    assert!(
        pl.vertices[..pl.vertices.len() - 1]
            .iter()
            .all(|v| v.bulge > 0.0)
    );
    for p in along(pl) {
        assert!(
            (p.x.hypot(p.y) - 1.0).abs() < 1e-6,
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
fn ellipse_becomes_a_closed_polyline_of_arcs_within_the_tolerance() {
    let text = dxf(
        None,
        "",
        &ent(
            "ELLIPSE",
            &[(10, "0"), (20, "0"), (11, "10"), (21, "0"), (40, "0.5")],
        ),
    );
    let d = read(&text);
    let Shape::Polyline(pl) = &d.shapes[0].1 else {
        panic!("{:?}", d.shapes[0].1)
    };
    assert!(pl.closed);
    assert!(
        pl.vertices.iter().all(|v| v.bulge != 0.0),
        "arcs, not lines: {pl:?}"
    );
    let ellipse = Curve::Ellipse {
        center: Point::new(0.0, 0.0),
        major_axis: Point::new(10.0, 0.0),
        ratio: 0.5,
        start_param: 0.0,
        end_param: TAU,
    };
    assert_within(pl, &ellipse, 0.01);
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
    let d = read(&text);
    let Shape::Polyline(pl) = &d.shapes[0].1 else {
        panic!("{:?}", d.shapes[0].1)
    };
    assert!(!pl.closed);
    let (first, last) = (pl.vertices[0], *pl.vertices.last().unwrap());
    assert!(
        first.x.abs() < 1e-9 && (first.y + 5.0).abs() < 1e-9,
        "{first:?}"
    );
    assert!(
        (last.x - 10.0).abs() < 1e-9 && last.y.abs() < 1e-9,
        "{last:?}"
    );
}

fn read_with(text: &str, scale: f64, curve_tolerance: f64) -> DxfDrawing {
    read_dxf(
        text,
        &DxfOptions {
            scale,
            curve_tolerance,
        },
    )
    .expect("valid DXF")
}

fn only_polyline(d: &DxfDrawing) -> &Polyline {
    match &d.shapes[..] {
        [(_, Shape::Polyline(pl))] => pl,
        other => panic!("{other:?}"),
    }
}

fn ellipse_dxf() -> String {
    dxf(
        None,
        "",
        &ent(
            "ELLIPSE",
            &[(10, "0"), (20, "0"), (11, "10"), (21, "0"), (40, "0.5")],
        ),
    )
}

#[test]
fn a_tighter_tolerance_gives_more_arcs_and_stays_within_it() {
    let ellipse = Curve::Ellipse {
        center: Point::new(0.0, 0.0),
        major_axis: Point::new(10.0, 0.0),
        ratio: 0.5,
        start_param: 0.0,
        end_param: TAU,
    };
    let coarse = read_with(&ellipse_dxf(), 1.0, 0.01);
    let fine = read_with(&ellipse_dxf(), 1.0, 0.0001);
    let (c, f) = (only_polyline(&coarse), only_polyline(&fine));
    assert!(
        f.vertices.len() > c.vertices.len(),
        "{} vs {}",
        f.vertices.len(),
        c.vertices.len()
    );
    assert_within(f, &ellipse, 0.0001);
}

#[test]
fn the_tolerance_is_in_mm_after_scaling_from_inches() {
    // A 10 × 5 inch ellipse imported as inches: within 0.01 mm of the 254 × 127 mm ellipse.
    let d = read_with(&ellipse_dxf(), 25.4, 0.01);
    let mm = Curve::Ellipse {
        center: Point::new(0.0, 0.0),
        major_axis: Point::new(254.0, 0.0),
        ratio: 0.5,
        start_param: 0.0,
        end_param: TAU,
    };
    assert_within(only_polyline(&d), &mm, 0.01);
}

/// An open cubic spline with an S bend (uniform clamped knots, 6 control points).
fn s_curve() -> curve::Spline {
    let p = Point::new;
    curve::Spline {
        degree: 3,
        control_points: vec![
            p(0.0, 0.0),
            p(10.0, 30.0),
            p(25.0, 30.0),
            p(35.0, -20.0),
            p(50.0, -20.0),
            p(60.0, 10.0),
        ],
        knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0, 3.0, 3.0],
        weights: Vec::new(),
        closed: false,
    }
}

fn spline_dxf(s: &curve::Spline) -> String {
    let mut pairs: Vec<(i32, String)> = vec![
        (70, if s.closed { "1" } else { "0" }.into()),
        (71, s.degree.to_string()),
    ];
    pairs.extend(s.knots.iter().map(|k| (40, k.to_string())));
    for c in &s.control_points {
        pairs.push((10, c.x.to_string()));
        pairs.push((20, c.y.to_string()));
    }
    let pairs: Vec<(i32, &str)> = pairs.iter().map(|(c, v)| (*c, v.as_str())).collect();
    dxf(None, "", &ent("SPLINE", &pairs))
}

#[test]
fn a_cubic_spline_becomes_arcs_within_the_tolerance() {
    let s = s_curve();
    for tol in [0.01, 0.001] {
        let d = read_with(&spline_dxf(&s), 1.0, tol);
        let pl = only_polyline(&d);
        assert!(!pl.closed);
        // Both bend directions are present (arcs turning left and right).
        assert!(pl.vertices.iter().any(|v| v.bulge > 0.0));
        assert!(pl.vertices.iter().any(|v| v.bulge < 0.0));
        assert_within(pl, &Curve::Spline(s.clone()), tol);
    }
}

#[test]
fn a_spline_corner_stays_sharp() {
    // A degree-1 spline is a set of straight lines: it becomes the same lines, corners kept.
    let p = Point::new;
    let s = curve::Spline {
        degree: 1,
        control_points: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)],
        knots: vec![0.0, 0.0, 1.0, 2.0, 2.0],
        weights: Vec::new(),
        closed: false,
    };
    let d = read(&spline_dxf(&s));
    let pts: Vec<(f64, f64, f64)> = only_polyline(&d)
        .vertices
        .iter()
        .map(|v| (v.x, v.y, v.bulge))
        .collect();
    assert_eq!(pts, [(0.0, 0.0, 0.0), (10.0, 0.0, 0.0), (10.0, 10.0, 0.0)]);
}

#[test]
fn a_closed_spline_becomes_a_closed_polyline() {
    // A periodic-looking cubic whose first and last control points are the same.
    let p = Point::new;
    let s = curve::Spline {
        degree: 3,
        control_points: vec![
            p(0.0, 0.0),
            p(20.0, -10.0),
            p(40.0, 10.0),
            p(20.0, 30.0),
            p(-10.0, 20.0),
            p(0.0, 0.0),
        ],
        knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0, 3.0, 3.0],
        weights: Vec::new(),
        closed: true,
    };
    let d = read(&spline_dxf(&s));
    let pl = only_polyline(&d);
    assert!(pl.closed);
    let (first, last) = (pl.vertices[0].point(), pl.vertices.last().unwrap().point());
    assert!(
        first.distance_to(last) > 1e-6,
        "the repeated end point is dropped"
    );
    assert_within(pl, &Curve::Spline(s), 0.01);
}

#[test]
fn a_tolerance_or_scale_of_zero_is_refused() {
    for (scale, tol) in [(1.0, 0.0), (0.0, 0.01), (1.0, f64::NAN), (1.0, -1.0)] {
        assert_eq!(
            read_dxf(
                &ellipse_dxf(),
                &DxfOptions {
                    scale,
                    curve_tolerance: tol
                }
            ),
            Err(DxfError::BadOptions)
        );
    }
}
