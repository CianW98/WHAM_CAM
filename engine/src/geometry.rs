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

/// An exact NURBS curve, kept as imported. Converted to arcs/lines only when a toolpath is made.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spline {
    pub degree: usize,
    pub control_points: Vec<Point>,
    pub knots: Vec<f64>,
    /// One weight per control point. Empty means all weights are 1 (a non-rational spline).
    #[serde(default)]
    pub weights: Vec<f64>,
    pub closed: bool,
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
    /// A list of points (e.g. hole centres for drilling).
    Points {
        points: Vec<Point>,
    },
    Spline(Spline),
    /// Point at parameter t = center + major_axis·cos t + minor_axis·sin t, where minor_axis is
    /// major_axis turned 90° counter-clockwise and scaled by `ratio`. Runs counter-clockwise
    /// from `start_param` to `end_param` (radians; 0 to 2π for a full ellipse).
    Ellipse {
        center: Point,
        major_axis: Point,
        ratio: f64,
        start_param: f64,
        end_param: f64,
    },
}

impl Shape {
    /// Name shown in the object tree, using common CAM names.
    pub fn type_name(&self) -> &'static str {
        match self {
            Shape::Line { .. } => "Line",
            Shape::Arc { .. } => "Arc",
            Shape::Circle { .. } => "Circle",
            Shape::Polyline(_) => "Polyline",
            Shape::Points { .. } => "PointList",
            Shape::Spline(_) => "Spline",
            Shape::Ellipse { .. } => "Ellipse",
        }
    }
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

    fn mirrors(&self) -> bool {
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

/// Angle of `p` seen from `center`, in degrees, in [0, 360).
fn angle_deg(center: Point, p: Point) -> f64 {
    (p.y - center.y)
        .atan2(p.x - center.x)
        .to_degrees()
        .rem_euclid(360.0)
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
            Shape::Spline(s) => Shape::Spline(Spline {
                control_points: s.control_points.iter().map(|p| t.apply(*p)).collect(),
                ..s.clone()
            }),
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
                let at = |deg: f64| {
                    let (sn, cs) = deg.to_radians().sin_cos();
                    t.apply(Point::new(center.x + radius * cs, center.y + radius * sn))
                };
                let c = t.apply(*center);
                let (p0, p1) = (at(*start_angle), at(*end_angle));
                // A mirror reverses direction; swap ends to keep the arc counter-clockwise.
                let from = if t.mirrors() { p1 } else { p0 };
                let sweep = (end_angle - start_angle).rem_euclid(360.0);
                let start = angle_deg(c, from);
                let end = if sweep == 0.0 {
                    start + 360.0
                } else {
                    start + sweep
                };
                Shape::Arc {
                    center: c,
                    radius: radius * s,
                    start_angle: start,
                    end_angle: end,
                }
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
            Shape::Ellipse {
                center,
                major_axis,
                ratio,
                start_param,
                end_param,
            } => {
                uniform?;
                let (start, end) = if t.mirrors() {
                    // Mirroring runs the ellipse the other way: parameter t becomes -t.
                    (-end_param, -start_param)
                } else {
                    (*start_param, *end_param)
                };
                let shift = if start < 0.0 { TAU } else { 0.0 };
                Shape::Ellipse {
                    center: t.apply(*center),
                    major_axis: t.apply_vector(*major_axis),
                    ratio: *ratio,
                    start_param: start + shift,
                    end_param: end + shift,
                }
            }
        })
    }
}
