//! Basic 2D geometry. All units are millimetres. Angles in shapes are degrees,
//! measured counter-clockwise from the +X axis (as in DXF).

use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance_to(self, other: Point) -> f64 {
        (other.x - self.x).hypot(other.y - self.y)
    }
}

/// A polyline corner. `bulge` describes the segment from this vertex to the next one,
/// the same way DXF does: 0 = straight line, otherwise tan(sweep angle / 4).
/// Positive bulge = counter-clockwise arc, negative = clockwise.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vertex {
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub bulge: f64,
}

impl Vertex {
    pub fn line(x: f64, y: f64) -> Self {
        Self { x, y, bulge: 0.0 }
    }

    pub fn point(self) -> Point {
        Point::new(self.x, self.y)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Polyline {
    pub vertices: Vec<Vertex>,
    pub closed: bool,
}

impl Polyline {
    /// The same path walked the other way. Arcs turn the other way, so bulges change sign.
    pub fn reversed(&self) -> Polyline {
        let v = &self.vertices;
        let n = v.len();
        let vertices = (0..n)
            .rev()
            .map(|k| {
                // The new segment from v[k] runs back to v[k-1] (to v[n-1] from v[0] if closed).
                let bulge = if k > 0 {
                    -v[k - 1].bulge
                } else if self.closed {
                    -v[n - 1].bulge
                } else {
                    0.0
                };
                Vertex {
                    x: v[k].x,
                    y: v[k].y,
                    bulge,
                }
            })
            .collect();
        Polyline {
            vertices,
            closed: self.closed,
        }
    }

    /// Axis-aligned rectangle with its lower-left corner at (x, y), drawn counter-clockwise.
    pub fn rectangle(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            vertices: vec![
                Vertex::line(x, y),
                Vertex::line(x + width, y),
                Vertex::line(x + width, y + height),
                Vertex::line(x, y + height),
            ],
            closed: true,
        }
    }
}

/// A drawing shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Shape {
    Line {
        start: Point,
        end: Point,
    },
    /// Counter-clockwise from `start_angle` to `end_angle` (degrees).
    Arc {
        center: Point,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    },
    Circle {
        center: Point,
        radius: f64,
    },
    Polyline(Polyline),
    /// A rectangle ("PolyRectangle"): `width` along the direction `rotation` degrees
    /// counter-clockwise from +X, `height` at right angles to it, from the `lower_left` corner
    /// (the lower-left corner before rotating). Always counter-clockwise. Build it with
    /// [`Shape::rectangle`] so axis-aligned results are stored with rotation 0.
    Rectangle {
        lower_left: Point,
        width: f64,
        height: f64,
        #[serde(default)]
        rotation: f64,
    },
    /// A list of points (e.g. hole centres for drilling).
    Points {
        points: Vec<Point>,
    },
    /// A dimension (measure only: never machined).
    Dimension(crate::dimension::Dimension),
}

impl Shape {
    /// Name shown in the object tree, using common CAM names.
    pub fn type_name(&self) -> &'static str {
        match self {
            Shape::Line { .. } => "Line",
            Shape::Arc { .. } => "Arc",
            Shape::Circle { .. } => "Circle",
            Shape::Polyline(_) => "Polyline",
            Shape::Rectangle { .. } => "PolyRectangle",
            Shape::Points { .. } => "PointList",
            Shape::Dimension(_) => "Dimension",
        }
    }
}

impl Shape {
    /// A rectangle. A rotation that is (to within rounding) a multiple of 90° is stored as an
    /// unrotated rectangle with its true lower-left corner, so "Lower Left" stays meaningful.
    pub fn rectangle(lower_left: Point, width: f64, height: f64, rotation: f64) -> Shape {
        let quarter = (rotation / 90.0).round();
        if (rotation - quarter * 90.0).abs() < 1e-7 {
            let c = rectangle_corners(lower_left, width, height, quarter * 90.0);
            let min_x = c.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
            let min_y = c.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
            let max_x = c.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
            let max_y = c.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
            return Shape::Rectangle {
                lower_left: Point::new(min_x, min_y),
                width: max_x - min_x,
                height: max_y - min_y,
                rotation: 0.0,
            };
        }
        Shape::Rectangle {
            lower_left,
            width,
            height,
            rotation: rotation.rem_euclid(360.0),
        }
    }
}

/// The four corners of a rectangle, counter-clockwise from the lower-left corner.
/// Exact multiples of 90° use exact sines and cosines, so axis-aligned corners have no rounding.
pub fn rectangle_corners(lower_left: Point, width: f64, height: f64, rotation: f64) -> [Point; 4] {
    let quarter = rotation / 90.0;
    let (s, c) = if quarter == quarter.round() {
        match (quarter as i64).rem_euclid(4) {
            0 => (0.0, 1.0),
            1 => (1.0, 0.0),
            2 => (0.0, -1.0),
            _ => (-1.0, 0.0),
        }
    } else {
        rotation.to_radians().sin_cos()
    };
    let along = Point::new(c * width, s * width);
    let up = Point::new(-s * height, c * height);
    let p = lower_left;
    [
        p,
        Point::new(p.x + along.x, p.y + along.y),
        Point::new(p.x + along.x + up.x, p.y + along.y + up.y),
        Point::new(p.x + up.x, p.y + up.y),
    ]
}

/// A 2D affine transform: x' = a·x + b·y + e, y' = c·x + d·y + f.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Transform {
    pub const IDENTITY: Transform = Transform {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    pub fn translate(x: f64, y: f64) -> Self {
        Self {
            e: x,
            f: y,
            ..Self::IDENTITY
        }
    }

    pub fn scale(sx: f64, sy: f64) -> Self {
        Self {
            a: sx,
            d: sy,
            ..Self::IDENTITY
        }
    }

    pub fn rotate_degrees(angle: f64) -> Self {
        let (s, c) = angle.to_radians().sin_cos();
        Self {
            a: c,
            b: -s,
            c: s,
            d: c,
            ..Self::IDENTITY
        }
    }

    /// Mirror in the Y axis (x → -x).
    pub fn mirror_x() -> Self {
        Self::scale(-1.0, 1.0)
    }

    /// Mirror in the X axis (y → -y).
    pub fn mirror_y() -> Self {
        Self::scale(1.0, -1.0)
    }

    /// `self.then(other)` applies `self` first, then `other`.
    pub fn then(self, o: Transform) -> Transform {
        Transform {
            a: o.a * self.a + o.b * self.c,
            b: o.a * self.b + o.b * self.d,
            c: o.c * self.a + o.d * self.c,
            d: o.c * self.b + o.d * self.d,
            e: o.a * self.e + o.b * self.f + o.e,
            f: o.c * self.e + o.d * self.f + o.f,
        }
    }

    pub fn apply(&self, p: Point) -> Point {
        Point::new(
            self.a * p.x + self.b * p.y + self.e,
            self.c * p.x + self.d * p.y + self.f,
        )
    }

    /// Applies only the rotation/scale part (for direction vectors).
    pub fn apply_vector(&self, v: Point) -> Point {
        Point::new(self.a * v.x + self.b * v.y, self.c * v.x + self.d * v.y)
    }

    fn det(&self) -> f64 {
        self.a * self.d - self.b * self.c
    }

    pub(crate) fn mirrors(&self) -> bool {
        self.det() < 0.0
    }

    /// If this transform keeps circles circular (uniform scale, rotation, mirror, move),
    /// returns the scale factor.
    pub fn uniform_scale(&self) -> Option<f64> {
        let sx = self.a.hypot(self.c);
        let sy = self.b.hypot(self.d);
        let dot = self.a * self.b + self.c * self.d;
        let tol = 1e-9 * sx.max(sy).max(1.0);
        ((sx - sy).abs() <= tol && dot.abs() <= tol && sx > 0.0).then_some(sx)
    }
}

// --- Shared point and arc maths ------------------------------------------------------------------
// Every file that works with arcs uses these, so they all agree (e.g. on how an arc that crosses
// 0° sweeps). Angles stored in shapes are degrees; the bulge maths works in radians.

/// `a − b`.
pub fn sub(a: Point, b: Point) -> Point {
    Point::new(a.x - b.x, a.y - b.y)
}

/// The point halfway between `a` and `b`.
pub fn mid(a: Point, b: Point) -> Point {
    Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
}

/// Cross product (z part): positive if `b` is anticlockwise from `a`.
pub fn cross(a: Point, b: Point) -> f64 {
    a.x * b.y - a.y * b.x
}

pub fn dot(a: Point, b: Point) -> f64 {
    a.x * b.x + a.y * b.y
}

/// The point on a circle at `angle` radians anticlockwise from +X.
pub fn on_circle(center: Point, radius: f64, angle: f64) -> Point {
    let (s, c) = angle.sin_cos();
    Point::new(center.x + radius * c, center.y + radius * s)
}

/// An angle in degrees brought into [0, 360). A value a hair under 360 (rounding noise) is 0.
pub fn norm_deg(a: f64) -> f64 {
    let a = a.rem_euclid(360.0);
    if 360.0 - a < 1e-9 { 0.0 } else { a }
}

/// Angle of `p` seen from `center`, in degrees, in [0, 360).
pub fn angle_deg(center: Point, p: Point) -> f64 {
    norm_deg((p.y - center.y).atan2(p.x - center.x).to_degrees())
}

/// How far an arc stored as start and end angles (degrees) sweeps, anticlockwise from start to
/// end, in degrees in (0, 360]. Crossing 0° is fine (300° to 30° sweeps 90°). Equal angles
/// (or within 1e-9°) mean a full circle.
pub fn arc_sweep_deg(start_angle: f64, end_angle: f64) -> f64 {
    let s = (end_angle - start_angle).rem_euclid(360.0);
    if s < 1e-9 { 360.0 } else { s }
}

/// The arc of a polyline segment with a bulge. Angles in radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BulgeArc {
    pub center: Point,
    pub radius: f64,
    /// Angle of the segment's start point seen from the centre.
    pub start: f64,
    /// Signed sweep: positive = anticlockwise (bulge > 0), negative = clockwise.
    pub sweep: f64,
}

impl BulgeArc {
    /// The point on the arc halfway along.
    pub fn mid_point(&self) -> Point {
        on_circle(self.center, self.radius, self.start + self.sweep / 2.0)
    }
}

/// The arc of the polyline segment from `a` to `b` with `bulge` (tan of a quarter of the
/// sweep). `None` for a straight segment (bulge 0) or if `a` and `b` are the same point.
pub fn bulge_arc(a: Point, b: Point, bulge: f64) -> Option<BulgeArc> {
    let chord = a.distance_to(b);
    if bulge == 0.0 || chord == 0.0 {
        return None;
    }
    // Distance from the chord's midpoint to the centre, towards the left of a→b.
    let offset = chord / 2.0 * (1.0 - bulge * bulge) / (2.0 * bulge);
    let (dx, dy) = ((b.x - a.x) / chord, (b.y - a.y) / chord);
    let m = mid(a, b);
    let center = Point::new(m.x - dy * offset, m.y + dx * offset);
    Some(BulgeArc {
        center,
        radius: center.distance_to(a),
        start: (a.y - center.y).atan2(a.x - center.x),
        sweep: 4.0 * bulge.atan(),
    })
}

/// The point a fraction `f` (0 to 1) of the way along the polyline segment from `s` to `e`.
pub fn point_along_segment(s: Point, e: Point, bulge: f64, f: f64) -> Point {
    match bulge_arc(s, e, bulge) {
        Some(arc) => on_circle(arc.center, arc.radius, arc.start + arc.sweep * f),
        None => Point::new(s.x + (e.x - s.x) * f, s.y + (e.y - s.y) * f),
    }
}

/// Distance from `p` to the polyline segment from `s` to `e` with `bulge` (straight when 0).
pub fn distance_to_segment(p: Point, s: Point, e: Point, bulge: f64) -> f64 {
    if let Some(arc) = bulge_arc(s, e, bulge) {
        // Is p within the arc's sweep (seen from the centre)?
        let ang = (p.y - arc.center.y).atan2(p.x - arc.center.x);
        let rel = (ang - arc.start).rem_euclid(TAU);
        let along = if arc.sweep >= 0.0 { rel } else { TAU - rel };
        if rel == 0.0 || along <= arc.sweep.abs() {
            return (p.distance_to(arc.center) - arc.radius).abs();
        }
        return p.distance_to(s).min(p.distance_to(e));
    }
    let d = sub(e, s);
    let len2 = dot(d, d);
    if len2 == 0.0 {
        return p.distance_to(s);
    }
    let f = (dot(sub(p, s), d) / len2).clamp(0.0, 1.0);
    p.distance_to(Point::new(s.x + d.x * f, s.y + d.y * f))
}

/// The bulge of an arc segment that sweeps `sweep` radians (positive = anticlockwise).
pub fn bulge_from_sweep(sweep: f64) -> f64 {
    (sweep / 4.0).tan()
}

impl Shape {
    /// Returns the transformed shape, or `None` if the result can't be represented exactly
    /// (e.g. a circle squashed by a non-uniform scale would become an ellipse).
    pub fn transformed(&self, t: &Transform) -> Option<Shape> {
        let uniform = t.uniform_scale();
        Some(match self {
            Shape::Line { start, end } => Shape::Line {
                start: t.apply(*start),
                end: t.apply(*end),
            },
            Shape::Points { points } => Shape::Points {
                points: points.iter().map(|p| t.apply(*p)).collect(),
            },
            Shape::Dimension(d) => Shape::Dimension(d.transformed(t)?),
            Shape::Circle { center, radius } => Shape::Circle {
                center: t.apply(*center),
                radius: radius * uniform?,
            },
            Shape::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                let s = uniform?;
                let at = |deg: f64| t.apply(on_circle(*center, *radius, deg.to_radians()));
                let c = t.apply(*center);
                let (p0, p1) = (at(*start_angle), at(*end_angle));
                // A mirror reverses direction; swap ends to keep the arc counter-clockwise.
                let from = if t.mirrors() { p1 } else { p0 };
                let start = angle_deg(c, from);
                let end = start + arc_sweep_deg(*start_angle, *end_angle);
                Shape::Arc {
                    center: c,
                    radius: radius * s,
                    start_angle: start,
                    end_angle: end,
                }
            }
            Shape::Rectangle {
                lower_left,
                width,
                height,
                rotation,
            } => {
                let s = uniform?;
                let c =
                    rectangle_corners(*lower_left, *width, *height, *rotation).map(|p| t.apply(p));
                // A mirror reverses the corner order; start from the old second corner so the
                // rectangle still runs counter-clockwise.
                let (origin, along) = if t.mirrors() {
                    (c[1], c[0])
                } else {
                    (c[0], c[1])
                };
                Shape::rectangle(origin, width * s, height * s, angle_deg(origin, along))
            }
            Shape::Polyline(pl) => {
                let has_arcs = pl.vertices.iter().any(|v| v.bulge != 0.0);
                if has_arcs {
                    uniform?;
                }
                let flip = if t.mirrors() { -1.0 } else { 1.0 };
                Shape::Polyline(Polyline {
                    vertices: pl
                        .vertices
                        .iter()
                        .map(|v| {
                            let p = t.apply(v.point());
                            Vertex {
                                x: p.x,
                                y: p.y,
                                bulge: v.bulge * flip,
                            }
                        })
                        .collect(),
                    closed: pl.closed,
                })
            }
        })
    }
}
