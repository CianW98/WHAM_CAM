//! SVG import (spec 5.1). Each test builds a tiny SVG, so the expected result is easy to check
//! by hand. Pages are 100 × 50 mm unless said otherwise: SVG's Y runs down from the top, so a
//! point at SVG y lands at mm y = 50 − y.

use wham_engine::svg::{SvgDrawing, SvgOptions};
use wham_engine::*;

const EPS: f64 = 1e-9;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < EPS
}

/// An Inkscape-style file: 100 × 50 mm page, viewBox in mm, with `body` inside.
fn inkscape(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg"
     xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape"
     width="100mm" height="50mm" viewBox="0 0 100 50">{body}</svg>"#
    )
}

fn read(text: &str) -> SvgDrawing {
    read_with(text, 1.0, 0.01)
}

fn read_with(text: &str, mm_per_unit: f64, tol: f64) -> SvgDrawing {
    read_svg(
        text,
        &SvgOptions {
            mm_per_unit,
            curve_tolerance: tol,
            default_layer: "drawing".into(),
        },
    )
    .expect("valid SVG")
}

fn only(d: &SvgDrawing) -> &Shape {
    assert_eq!(d.shapes.len(), 1, "{:?}", d.shapes);
    &d.shapes[0].1
}

fn polyline(d: &SvgDrawing) -> &Polyline {
    match only(d) {
        Shape::Polyline(pl) => pl,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_circle_stays_a_circle_flipped_onto_the_page() {
    let d = read(&inkscape(r#"<circle cx="10" cy="10" r="5"/>"#));
    assert!(d.size_known);
    assert!(close(d.mm_per_unit, 1.0));
    let Shape::Circle { center, radius } = only(&d) else {
        panic!()
    };
    assert!(close(center.x, 10.0) && close(center.y, 40.0), "{center:?}");
    assert!(close(*radius, 5.0));
    assert_eq!(d.shapes[0].0, "drawing");
}

#[test]
fn a_rect_stays_a_polyrectangle() {
    let d = read(&inkscape(r#"<rect x="10" y="5" width="20" height="10"/>"#));
    let Shape::Rectangle {
        lower_left,
        width,
        height,
        rotation,
    } = only(&d)
    else {
        panic!("{:?}", d.shapes)
    };
    // SVG top edge y = 5, bottom edge y = 15 → mm 35 to 45.
    assert!(
        close(lower_left.x, 10.0) && close(lower_left.y, 35.0),
        "{lower_left:?}"
    );
    assert!(close(*width, 20.0) && close(*height, 10.0) && close(*rotation, 0.0));
}

#[test]
fn a_rounded_rect_has_true_corner_arcs() {
    let d = read(&inkscape(
        r#"<rect x="0" y="0" width="40" height="20" rx="5"/>"#,
    ));
    let pl = polyline(&d);
    assert!(pl.closed);
    let arcs: Vec<f64> = pl
        .vertices
        .iter()
        .map(|v| v.bulge)
        .filter(|b| *b != 0.0)
        .collect();
    assert_eq!(arcs.len(), 4, "{pl:?}");
    // Quarter circles: bulge tan(90°/4), all turning the same way.
    let q = (std::f64::consts::FRAC_PI_2 / 4.0).tan();
    assert!(
        arcs.iter()
            .all(|b| close(b.abs(), q) && b.signum() == arcs[0].signum())
    );
    assert_eq!(d.curves, 0);
}

#[test]
fn a_circular_arc_in_a_path_is_a_true_arc() {
    // From (0, 10) to (20, 10), centre (10, 10), bulging up the page (SVG y = 0).
    let d = read(&inkscape(r#"<path d="M 0 10 A 10 10 0 0 1 20 10"/>"#));
    let pl = polyline(&d);
    assert!(!pl.closed);
    let v = &pl.vertices;
    assert_eq!(v.len(), 2);
    assert!(
        close(v[0].x, 0.0) && close(v[0].y, 40.0) && close(v[1].x, 20.0) && close(v[1].y, 40.0)
    );
    // Going right over the top is clockwise: a half circle with bulge −1.
    assert!(close(v[0].bulge, -1.0), "{}", v[0].bulge);
    assert_eq!(d.curves, 0);
}

#[test]
fn relative_lines_and_close_make_a_closed_polyline() {
    let d = read(&inkscape(r#"<path d="m 10 10 h 20 v 10 l -20 0 z"/>"#));
    let pl = polyline(&d);
    assert!(pl.closed);
    let pts: Vec<(f64, f64)> = pl.vertices.iter().map(|v| (v.x, v.y)).collect();
    assert_eq!(
        pts,
        [(10.0, 40.0), (30.0, 40.0), (30.0, 30.0), (10.0, 30.0)]
    );
}

#[test]
fn polygons_polylines_and_lines() {
    let d = read(&inkscape(
        r#"<polygon points="0,0 10,0 10,10"/><polyline points="0 0, 5 5, 10 0"/><line x1="1" y1="2" x2="3" y2="4"/>"#,
    ));
    assert_eq!(d.shapes.len(), 3);
    let closed: Vec<bool> = d
        .shapes
        .iter()
        .map(|(_, s)| matches!(s, Shape::Polyline(pl) if pl.closed))
        .collect();
    assert_eq!(closed, [true, false, false]);
}

#[test]
fn a_bezier_becomes_arcs_within_the_tolerance_ending_exactly() {
    let d = read_with(
        &inkscape(r#"<path d="M 0 25 C 30 0 70 50 100 25"/>"#),
        1.0,
        0.01,
    );
    assert_eq!(d.curves, 1);
    let pl = polyline(&d);
    assert!(pl.vertices.len() > 2);
    assert!(pl.vertices.iter().any(|v| v.bulge != 0.0));
    let (first, last) = (pl.vertices[0], *pl.vertices.last().unwrap());
    assert!(close(first.x, 0.0) && close(first.y, 25.0));
    assert!(close(last.x, 100.0) && close(last.y, 25.0));
    // Every vertex lies on the curve (within the tolerance): check against the Bézier sampled
    // finely, in mm (y flipped).
    let bez = |t: f64| {
        let u = 1.0 - t;
        let x = 3.0 * u * u * t * 30.0 + 3.0 * u * t * t * 70.0 + t * t * t * 100.0;
        let y =
            u * u * u * 25.0 + 3.0 * u * u * t * 0.0 + 3.0 * u * t * t * 50.0 + t * t * t * 25.0;
        Point::new(x, 50.0 - y)
    };
    for v in &pl.vertices {
        let d = (0..=20000)
            .map(|i| bez(i as f64 / 20000.0).distance_to(v.point()))
            .fold(f64::INFINITY, f64::min);
        assert!(d < 0.01 + 1e-4, "{v:?} is {d} from the curve");
    }
}

#[test]
fn an_ellipse_becomes_a_closed_polyline_and_counts_as_a_curve() {
    let d = read(&inkscape(r#"<ellipse cx="50" cy="25" rx="20" ry="10"/>"#));
    assert_eq!(d.curves, 1);
    let pl = polyline(&d);
    assert!(pl.closed);
    for v in &pl.vertices {
        let e = ((v.x - 50.0) / 20.0).powi(2) + ((v.y - 25.0) / 10.0).powi(2);
        assert!((e - 1.0).abs() < 0.01, "{v:?}");
    }
}

#[test]
fn transforms_are_followed_and_uneven_scales_turn_circles_into_curves() {
    let d = read(&inkscape(
        r#"<g transform="translate(30,0)"><circle transform="rotate(90)" cx="10" cy="0" r="2"/></g>
           <circle transform="scale(2,1)" cx="10" cy="10" r="2"/>"#,
    ));
    // rotate(90) takes (10, 0) to (0, 10); translate adds 30 → SVG (30, 10) → mm (30, 40).
    let Shape::Circle { center, radius } = &d.shapes[0].1 else {
        panic!("{:?}", d.shapes[0])
    };
    assert!(
        (center.x - 30.0).abs() < 1e-9 && (center.y - 40.0).abs() < 1e-9,
        "{center:?}"
    );
    assert!(close(*radius, 2.0));
    assert!(matches!(d.shapes[1].1, Shape::Polyline(_)));
    assert_eq!(d.curves, 1);
}

#[test]
fn inkscape_layers_become_layers_and_hidden_ones_are_left_out() {
    let d = read(&inkscape(
        r#"<g inkscape:groupmode="layer" inkscape:label="Holes"><circle cx="5" cy="5" r="1"/></g>
           <g inkscape:groupmode="layer" inkscape:label="Hidden" style="display:none"><circle cx="5" cy="5" r="1"/></g>
           <g><circle cx="9" cy="9" r="1"/></g>"#,
    ));
    let layers: Vec<&str> = d.shapes.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(layers, ["Holes", "drawing"]);
}

#[test]
fn unit_less_files_use_the_given_scale_and_report_it() {
    // 96 px per inch, as Inkscape and browsers use.
    let px = 25.4 / 96.0;
    let text = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100" viewBox="0 0 200 100">
        <circle cx="96" cy="50" r="96"/></svg>"#;
    let d = read_with(text, px, 0.01);
    assert!(!d.size_known);
    let Shape::Circle { center, radius } = only(&d) else {
        panic!()
    };
    assert!(close(*radius, 25.4) && close(center.x, 25.4) && close(center.y, 50.0 * px));
}

#[test]
fn size_from_inches_and_no_viewbox_means_css_pixels() {
    // width in inches, no viewBox: units are px (1/96 in), size known.
    let text = r#"<svg xmlns="http://www.w3.org/2000/svg" width="1in" height="1in"><rect width="96" height="48"/></svg>"#;
    let d = read_with(text, 1.0, 0.01);
    assert!(d.size_known);
    let Shape::Rectangle { width, height, .. } = only(&d) else {
        panic!()
    };
    assert!(close(*width, 25.4) && close(*height, 12.7));
}

#[test]
fn text_images_and_copies_are_skipped_and_listed() {
    let d = read(&inkscape(
        r##"<text x="1" y="1">Hi</text><image href="x.png"/><use href="#c"/><defs><circle id="c" r="1"/></defs>"##,
    ));
    assert!(d.shapes.is_empty());
    let skipped: Vec<(&str, usize)> = d.skipped.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    assert_eq!(skipped, [("image", 1), ("text", 1), ("use", 1)]);
}

#[test]
fn not_an_svg_is_an_error() {
    assert!(matches!(
        read_svg(
            "hello",
            &SvgOptions {
                mm_per_unit: 1.0,
                curve_tolerance: 0.01,
                default_layer: "x".into()
            }
        ),
        Err(SvgError::NotSvg(_))
    ));
    assert!(matches!(
        read_svg(
            "<html/>",
            &SvgOptions {
                mm_per_unit: 1.0,
                curve_tolerance: 0.01,
                default_layer: "x".into()
            }
        ),
        Err(SvgError::NotSvg(_))
    ));
}

#[test]
fn importing_into_a_project_adds_layers_as_one_undo_step() {
    let mut p = Project::new();
    let text = inkscape(
        r#"<g inkscape:groupmode="layer" inkscape:label="Outline"><rect width="10" height="10"/></g><path d="M0 0 C 10 0 10 10 0 10"/>"#,
    );
    let info = Project::svg_info(&text).unwrap();
    assert_eq!(
        (info.size_known, info.shape_count, info.curve_count),
        (true, 2, 1)
    );
    let r = p.import_svg(&text, 1.0, 0.01, "part").unwrap();
    assert_eq!(r.objects_added, 2);
    assert_eq!(r.layers, ["Outline", "part"]);
    assert_eq!(r.curves_converted, 1);
    assert!(p.undo());
    assert_eq!(p.document().objects().count(), 0);
}
