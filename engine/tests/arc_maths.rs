//! Tests for the shared point and arc maths in `geometry` that every other part relies on.

use std::f64::consts::{FRAC_PI_2, PI};

use wham_engine::geometry::{
    angle_deg, arc_sweep_deg, bulge_arc, bulge_from_sweep, norm_deg, on_circle,
};
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

#[test]
fn arc_sweep_is_anticlockwise_and_crosses_zero() {
    assert!(close(arc_sweep_deg(0.0, 90.0), 90.0));
    assert!(close(arc_sweep_deg(300.0, 30.0), 90.0));
    assert!(close(arc_sweep_deg(90.0, 0.0), 270.0));
    assert!(close(arc_sweep_deg(10.0, 370.0), 360.0)); // equal angles: a full circle
    assert!(close(arc_sweep_deg(45.0, 45.0), 360.0));
    assert!(close(arc_sweep_deg(-90.0, 90.0), 180.0));
}

#[test]
fn angles_normalise_into_0_to_360() {
    assert!(close(norm_deg(-90.0), 270.0));
    assert!(close(norm_deg(720.0 + 45.0), 45.0));
    assert_eq!(norm_deg(360.0 - 1e-12), 0.0); // rounding noise just under 360° is 0°
    assert!(close(angle_deg(p(1.0, 1.0), p(1.0, 0.0)), 270.0));
    assert!(close(angle_deg(p(0.0, 0.0), p(-1.0, 0.0)), 180.0));
    assert!(same(on_circle(p(1.0, 2.0), 3.0, FRAC_PI_2), p(1.0, 5.0)));
}

#[test]
fn bulge_one_is_a_half_circle_on_the_correct_side() {
    // From (0,0) to (10,0), bulge +1: anticlockwise half circle, which dips below the chord.
    let arc = bulge_arc(p(0.0, 0.0), p(10.0, 0.0), 1.0).unwrap();
    assert!(same(arc.center, p(5.0, 0.0)));
    assert!(close(arc.radius, 5.0));
    assert!(close(arc.sweep, PI));
    assert!(same(arc.mid_point(), p(5.0, -5.0)));
    // Bulge −1: clockwise, over the top.
    let arc = bulge_arc(p(0.0, 0.0), p(10.0, 0.0), -1.0).unwrap();
    assert!(close(arc.sweep, -PI));
    assert!(same(arc.mid_point(), p(5.0, 5.0)));
}

#[test]
fn bulge_arc_ends_on_both_points() {
    for bulge in [0.1, 0.5, 1.0, 2.5, -0.3, -1.0, -4.0] {
        let (a, b) = (p(3.0, -2.0), p(-7.0, 11.0));
        let arc = bulge_arc(a, b, bulge).unwrap();
        assert!(
            same(on_circle(arc.center, arc.radius, arc.start), a),
            "bulge {bulge}"
        );
        let end = on_circle(arc.center, arc.radius, arc.start + arc.sweep);
        assert!(same(end, b), "bulge {bulge}");
        assert!(close(bulge_from_sweep(arc.sweep), bulge), "bulge {bulge}");
    }
}

#[test]
fn straight_and_zero_length_segments_have_no_arc() {
    assert!(bulge_arc(p(0.0, 0.0), p(1.0, 0.0), 0.0).is_none());
    assert!(bulge_arc(p(1.0, 1.0), p(1.0, 1.0), 0.5).is_none());
}

#[test]
fn display_and_bounds_agree_on_an_arc_across_zero() {
    // 300° to 30°: the right-hand side of the circle.
    let arc = Shape::Arc {
        center: p(0.0, 0.0),
        radius: 10.0,
        start_angle: 300.0,
        end_angle: 30.0,
    };
    let b = shape_bounds(&arc).unwrap();
    assert!(
        close(b[2], 10.0),
        "passes through 0° so reaches x = 10: {b:?}"
    );
    assert!(b[0] > 4.9, "never reaches the left side: {b:?}");
    let shown = wham_engine::display::display_shape(1, &arc);
    assert!(
        shown
            .points
            .iter()
            .all(|q| q[0] >= b[0] - 1e-9 && q[0] <= b[2] + 1e-9)
    );
}

#[test]
fn svg_arc_center_finds_the_centre_angles_and_corrects_small_radii() {
    use wham_engine::geometry::svg_arc_center;
    let p = Point::new;
    // Half circle from (0,0) to (20,0), positive direction: centre (10,0), from 180° through +180°.
    let a = svg_arc_center(p(0.0, 0.0), p(20.0, 0.0), 10.0, 10.0, 0.0, false, true).unwrap();
    assert!((a.center.x - 10.0).abs() < 1e-12 && a.center.y.abs() < 1e-12);
    assert!((a.start.abs() - std::f64::consts::PI).abs() < 1e-12);
    assert!((a.sweep - std::f64::consts::PI).abs() < 1e-12);
    // Radius too small to reach: scaled up to half the distance.
    let b = svg_arc_center(p(0.0, 0.0), p(20.0, 0.0), 1.0, 1.0, 0.0, false, false).unwrap();
    assert!((b.rx - 10.0).abs() < 1e-12 && (b.sweep + std::f64::consts::PI).abs() < 1e-12);
    // Quarter arc, small vs large: the large one goes the long way round.
    let small = svg_arc_center(p(10.0, 0.0), p(0.0, 10.0), 10.0, 10.0, 0.0, false, true).unwrap();
    let large = svg_arc_center(p(10.0, 0.0), p(0.0, 10.0), 10.0, 10.0, 0.0, true, true).unwrap();
    assert!((small.sweep - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    assert!((large.sweep - 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    // A radius of 0, or the same point twice: no arc.
    assert!(svg_arc_center(p(0.0, 0.0), p(5.0, 0.0), 0.0, 3.0, 0.0, false, true).is_none());
    assert!(svg_arc_center(p(1.0, 1.0), p(1.0, 1.0), 3.0, 3.0, 0.0, false, true).is_none());
}

#[test]
fn elliptical_arc_spline_lies_exactly_on_the_ellipse() {
    use wham_engine::curve::{Curve, elliptical_arc};
    use wham_engine::geometry::EllipticalArc;
    let arc = EllipticalArc {
        center: Point::new(3.0, -2.0),
        rx: 20.0,
        ry: 7.0,
        phi: 0.4,
        start: -0.3,
        sweep: 4.5,
    };
    let c = Curve::Spline(elliptical_arc(&arc));
    let Curve::Spline(s) = &c else { unreachable!() };
    let (u0, u1) = (s.knots[s.degree], s.knots[s.control_points.len()]);
    for i in 0..=400 {
        let q = c.point(u0 + (u1 - u0) * i as f64 / 400.0);
        // Back into the ellipse's own frame: (x/rx)² + (y/ry)² = 1.
        let (dx, dy) = (q.x - arc.center.x, q.y - arc.center.y);
        let (sp, cp) = arc.phi.sin_cos();
        let (x, y) = (cp * dx + sp * dy, -sp * dx + cp * dy);
        let e = (x / arc.rx).powi(2) + (y / arc.ry).powi(2);
        assert!((e - 1.0).abs() < 1e-12, "{q:?}: {e}");
    }
    // Ends where it should.
    let end = c.point(u1);
    let (sp, cp) = arc.phi.sin_cos();
    let a = arc.start + arc.sweep;
    let want = Point::new(
        arc.center.x + cp * arc.rx * a.cos() - sp * arc.ry * a.sin(),
        arc.center.y + sp * arc.rx * a.cos() + cp * arc.ry * a.sin(),
    );
    assert!(end.distance_to(want) < 1e-12);
}
