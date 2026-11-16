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
