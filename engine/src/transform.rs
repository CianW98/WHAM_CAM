//! Moving, rotating, scaling and mirroring shapes, and exact bounding boxes.
//! Rotate, scale and mirror work around the centre of the selected shapes' bounding box.

use serde::{Deserialize, Serialize};

use crate::display::display_shape;
use crate::geometry::{Point, Shape, Transform, rectangle_corners};

/// One transform of the selected shapes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TransformOp {
    /// Move by (dx, dy) mm.
    Move { dx: f64, dy: f64 },
    /// Rotate by `degrees`, counter-clockwise, around the selection centre.
    Rotate { degrees: f64 },
    /// Rotate around the selection centre by the angle from the direction of `from` to the
    /// direction of `to` (both seen from the centre), as when rotating by clicking. If either
    /// point is on the centre there is no direction, and nothing turns.
    RotateFromTo { from: Point, to: Point },
    /// Scale by `factor` (more than 0) around the selection centre.
    Scale { factor: f64 },
    /// Flip across the vertical line through the selection centre.
    MirrorLeftRight,
    /// Flip across the horizontal line through the selection centre.
    MirrorTopBottom,
}

impl TransformOp {
    /// Checks the values. Errors are plain-English messages.
    pub fn check(&self) -> Result<(), String> {
        let ok = match *self {
            TransformOp::Move { dx, dy } => dx.is_finite() && dy.is_finite(),
            TransformOp::Rotate { degrees } => degrees.is_finite(),
            TransformOp::RotateFromTo { from, to } => {
                from.x.is_finite() && from.y.is_finite() && to.x.is_finite() && to.y.is_finite()
            }
            TransformOp::Scale { factor } => {
                if !(factor.is_finite() && factor > 0.0) {
                    return Err("The scale factor must be more than 0.".to_string());
                }
                true
            }
            TransformOp::MirrorLeftRight | TransformOp::MirrorTopBottom => true,
        };
        if ok {
            Ok(())
        } else {
            Err("Every value must be a number.".to_string())
        }
    }

    /// The transform, working around `centre`.
    pub fn to_transform(&self, centre: Point) -> Transform {
        let about = |t: Transform| {
            Transform::translate(-centre.x, -centre.y)
                .then(t)
                .then(Transform::translate(centre.x, centre.y))
        };
        match *self {
            TransformOp::Move { dx, dy } => Transform::translate(dx, dy),
            TransformOp::Rotate { degrees } => about(Transform::rotate_degrees(degrees)),
            TransformOp::RotateFromTo { from, to } => {
                about(Transform::rotate_degrees(angle_from_to(centre, from, to)))
            }
            TransformOp::Scale { factor } => about(Transform::scale(factor, factor)),
            TransformOp::MirrorLeftRight => about(Transform::mirror_x()),
            TransformOp::MirrorTopBottom => about(Transform::mirror_y()),
        }
    }
}

/// Angle in degrees (counter-clockwise) from the direction of `from` to the direction of `to`,
/// both seen from `centre`. 0 if either point is on the centre.
pub fn angle_from_to(centre: Point, from: Point, to: Point) -> f64 {
    let (fx, fy) = (from.x - centre.x, from.y - centre.y);
    let (tx, ty) = (to.x - centre.x, to.y - centre.y);
    if fx.hypot(fy) < 1e-9 || tx.hypot(ty) < 1e-9 {
        return 0.0;
    }
    (fx * ty - fy * tx).atan2(fx * tx + fy * ty).to_degrees()
}

/// An axis-aligned box, [min_x, min_y, max_x, max_y].
pub type Bounds = [f64; 4];

fn grow(b: &mut Option<Bounds>, p: Point) {
    let r = b.get_or_insert([p.x, p.y, p.x, p.y]);
    r[0] = r[0].min(p.x);
    r[1] = r[1].min(p.y);
    r[2] = r[2].max(p.x);
    r[3] = r[3].max(p.y);
}

/// Adds an arc's extent: its ends, plus the 0°/90°/180°/270° points it passes through.
/// `start` and `sweep` are in degrees; a negative sweep runs clockwise.
fn grow_arc(b: &mut Option<Bounds>, centre: Point, radius: f64, start: f64, sweep: f64) {
    let (start, sweep) = if sweep < 0.0 {
        (start + sweep, -sweep)
    } else {
        (start, sweep)
    };
    let at = |deg: f64| {
        let (s, c) = deg.to_radians().sin_cos();
        Point::new(centre.x + radius * c, centre.y + radius * s)
    };
    grow(b, at(start));
    grow(b, at(start + sweep));
    let first = (start / 90.0).ceil() as i64;
    let last = ((start + sweep) / 90.0).floor() as i64;
    // A full circle crosses each axis point at most once more; cap the loop for odd input.
    for k in first..=last.min(first + 4) {
        grow(b, at(k as f64 * 90.0));
    }
}

/// Centre, radius, start angle and sweep (degrees, signed) of a polyline bulge segment.
fn bulge_arc(a: Point, b: Point, bulge: f64) -> Option<(Point, f64, f64, f64)> {
    let (cx, cy) = (b.x - a.x, b.y - a.y);
    let len = cx.hypot(cy);
    if bulge == 0.0 || len == 0.0 {
        return None;
    }
    let n = Point::new(cy / len, -cx / len);
    let sagitta = bulge * len / 2.0;
    let radius = len * (1.0 + bulge * bulge) / (4.0 * bulge.abs());
    let d = sagitta - radius * bulge.signum();
    let centre = Point::new((a.x + b.x) / 2.0 + n.x * d, (a.y + b.y) / 2.0 + n.y * d);
    let start = (a.y - centre.y).atan2(a.x - centre.x).to_degrees();
    let sweep = 4.0 * bulge.atan().to_degrees();
    Some((centre, radius, start, sweep))
}

/// Exact bounding box of a shape (splines and ellipses: within the 0.01 mm display tolerance).
/// `None` for a shape with no points.
pub fn shape_bounds(shape: &Shape) -> Option<Bounds> {
    let mut b = None;
    match shape {
        Shape::Line { start, end } => {
            grow(&mut b, *start);
            grow(&mut b, *end);
        }
        Shape::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let sweep = (end_angle - start_angle).rem_euclid(360.0);
            let sweep = if sweep == 0.0 { 360.0 } else { sweep };
            grow_arc(&mut b, *center, *radius, *start_angle, sweep);
        }
        Shape::Circle { center, radius } => {
            grow(&mut b, Point::new(center.x - radius, center.y - radius));
            grow(&mut b, Point::new(center.x + radius, center.y + radius));
        }
        Shape::Polyline(pl) => {
            let n = pl.vertices.len();
            for v in &pl.vertices {
                grow(&mut b, v.point());
            }
            let segments = if pl.closed { n } else { n.saturating_sub(1) };
            for i in 0..segments {
                let (v, w) = (pl.vertices[i], pl.vertices[(i + 1) % n]);
                if let Some((c, r, start, sweep)) = bulge_arc(v.point(), w.point(), v.bulge) {
                    grow_arc(&mut b, c, r, start, sweep);
                }
            }
        }
        Shape::Points { points } => {
            for p in points {
                grow(&mut b, *p);
            }
        }
        Shape::Rectangle {
            lower_left,
            width,
            height,
            rotation,
        } => {
            for p in rectangle_corners(*lower_left, *width, *height, *rotation) {
                grow(&mut b, p);
            }
        }
        Shape::Spline(_) | Shape::Ellipse { .. } => {
            for p in display_shape(0, shape).points {
                grow(&mut b, Point::new(p[0], p[1]));
            }
        }
    }
    b
}

/// Bounding box of several shapes together.
pub fn bounds_of<'a>(shapes: impl IntoIterator<Item = &'a Shape>) -> Option<Bounds> {
    let mut b = None;
    for s in shapes {
        if let Some(r) = shape_bounds(s) {
            grow(&mut b, Point::new(r[0], r[1]));
            grow(&mut b, Point::new(r[2], r[3]));
        }
    }
    b
}

/// Centre of a bounding box.
pub fn centre(b: Bounds) -> Point {
    Point::new((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0)
}

/// Applies `op` to `shapes` around their common centre. Errors (plain English) if a shape
/// can't be transformed exactly or the values are bad; then nothing should be changed.
pub fn transform_shapes(shapes: &[Shape], op: TransformOp) -> Result<Vec<Shape>, String> {
    op.check()?;
    let Some(b) = bounds_of(shapes) else {
        return Ok(shapes.to_vec());
    };
    let t = op.to_transform(centre(b));
    shapes
        .iter()
        .map(|s| {
            let out = s
                .transformed(&t)
                .ok_or_else(|| format!("A {} can't be transformed this way.", s.type_name()))?;
            crate::draw::validate(&out)?;
            Ok(out)
        })
        .collect()
}
