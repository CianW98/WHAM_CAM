//! Dimensions (spec 5.3): aligned, radius, diameter and angle. A dimension only shows a size; it
//! never changes the geometry and is never machined. The value is measured when the dimension
//! is placed and isn't linked to the shape it measured. Moving, rotating and mirroring a
//! dimension keep its value; scaling multiplies a length by the factor (angles stay), so a
//! dimension always matches its own drawing (decided 2026-11-20).
//!
//! Placing (decided 2026-11-20):
//! - Aligned: two snapped points, then a third click sets where the dimension line sits.
//! - Radius / diameter: click an arc or circle, then click where the value goes.
//! - Angle: click two lines, then click inside the angle to measure (any of the four around the
//!   crossing); that click also sets the arc's radius.
//!
//! This module also lays out each dimension for the screen: its lines, arrowheads and label.

use std::f64::consts::{PI, TAU};

use serde::{Deserialize, Serialize};

use crate::geometry::{
    Point, Shape, Transform, angle_deg, bulge_arc, distance_to_segment, dot, on_circle,
    rectangle_corners, sub,
};

/// Text height of dimension values, in mm (one look in v1).
pub const TEXT_HEIGHT: f64 = 3.5;
/// Arrowhead length and half-width, in mm.
const ARROW_LENGTH: f64 = 2.5;
const ARROW_HALF_WIDTH: f64 = 0.6;
/// Extension lines start this far from the measured point and run this far past the
/// dimension line, in mm.
const EXTENSION_GAP: f64 = 1.0;
const EXTENSION_BEYOND: f64 = 1.5;
/// Space between a line and its label, in mm.
const LABEL_GAP: f64 = 1.0;
/// Rough width of one character as a fraction of the text height (for the label's box).
const CHAR_WIDTH: f64 = 0.6;
/// Points closer than this (mm) count as the same point.
const SAME_POINT: f64 = 1e-9;

/// A dimension, with its measured value (mm for lengths, degrees for angles).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Dimension {
    /// The distance from `from` to `to`, measured along the line between them. The dimension
    /// line runs parallel to it, through `at`.
    Aligned {
        from: Point,
        to: Point,
        at: Point,
        value: f64,
    },
    /// The radius of a circle or arc, with a leader from the centre towards `at`, where the
    /// value is written.
    Radius {
        center: Point,
        radius: f64,
        at: Point,
        value: f64,
    },
    /// The diameter of a circle or arc, across the centre towards `at`, where the value is
    /// written.
    Diameter {
        center: Point,
        radius: f64,
        at: Point,
        value: f64,
    },
    /// The angle at `vertex`, counter-clockwise from the direction of `start` to the direction
    /// of `end`. The arc is drawn at the distance of `start` from the vertex (`end` is at the
    /// same distance).
    Angle {
        vertex: Point,
        start: Point,
        end: Point,
        value: f64,
    },
}

/// The four kinds, for the drawing tools and the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DimensionKind {
    Aligned,
    Radius,
    Diameter,
    Angle,
}

impl Dimension {
    pub fn kind(&self) -> DimensionKind {
        match self {
            Dimension::Aligned { .. } => DimensionKind::Aligned,
            Dimension::Radius { .. } => DimensionKind::Radius,
            Dimension::Diameter { .. } => DimensionKind::Diameter,
            Dimension::Angle { .. } => DimensionKind::Angle,
        }
    }

    pub fn value(&self) -> f64 {
        match self {
            Dimension::Aligned { value, .. }
            | Dimension::Radius { value, .. }
            | Dimension::Diameter { value, .. }
            | Dimension::Angle { value, .. } => *value,
        }
    }

    /// The value as shown: 2 decimals, `R` for a radius, `Ø` for a diameter, `°` for an angle.
    pub fn text(&self) -> String {
        let v = self.value();
        match self {
            Dimension::Aligned { .. } => format!("{v:.2}"),
            Dimension::Radius { .. } => format!("R{v:.2}"),
            Dimension::Diameter { .. } => format!("Ø{v:.2}"),
            Dimension::Angle { .. } => format!("{v:.2}°"),
        }
    }

    /// Every point that defines the dimension (for checking it is valid).
    pub fn points(&self) -> Vec<Point> {
        match self {
            Dimension::Aligned { from, to, at, .. } => vec![*from, *to, *at],
            Dimension::Radius { center, at, .. } | Dimension::Diameter { center, at, .. } => {
                vec![*center, *at]
            }
            Dimension::Angle {
                vertex, start, end, ..
            } => vec![*vertex, *start, *end],
        }
    }

    /// Checks the numbers make a drawable dimension.
    pub fn problem(&self) -> Option<&'static str> {
        let finite = self
            .points()
            .iter()
            .all(|p| p.x.is_finite() && p.y.is_finite());
        if !finite || !self.value().is_finite() || self.value() < 0.0 {
            return Some("A value is not a number.");
        }
        match self {
            Dimension::Aligned { from, to, .. } if from.distance_to(*to) <= SAME_POINT => {
                Some("An aligned dimension needs two different points.")
            }
            Dimension::Radius { radius, .. } | Dimension::Diameter { radius, .. }
                if !(radius.is_finite() && *radius > 0.0) =>
            {
                Some("A radius must be more than 0.")
            }
            Dimension::Angle {
                vertex, start, end, ..
            } if vertex.distance_to(*start) <= SAME_POINT
                || vertex.distance_to(*end) <= SAME_POINT =>
            {
                Some("An angle dimension's arc needs a radius.")
            }
            _ => None,
        }
    }

    /// The transformed dimension, or `None` under an uneven scale (a circle would stop being
    /// one). Lengths scale with the transform; angles don't change.
    pub fn transformed(&self, t: &Transform) -> Option<Dimension> {
        let s = t.uniform_scale()?;
        Some(match self {
            Dimension::Aligned {
                from,
                to,
                at,
                value,
            } => Dimension::Aligned {
                from: t.apply(*from),
                to: t.apply(*to),
                at: t.apply(*at),
                value: value * s,
            },
            Dimension::Radius {
                center,
                radius,
                at,
                value,
            } => Dimension::Radius {
                center: t.apply(*center),
                radius: radius * s,
                at: t.apply(*at),
                value: value * s,
            },
            Dimension::Diameter {
                center,
                radius,
                at,
                value,
            } => Dimension::Diameter {
                center: t.apply(*center),
                radius: radius * s,
                at: t.apply(*at),
                value: value * s,
            },
            Dimension::Angle {
                vertex,
                start,
                end,
                value,
            } => {
                // A mirror reverses the turning direction: swap the ends to stay anticlockwise.
                let (a, b) = if t.mirrors() {
                    (end, start)
                } else {
                    (start, end)
                };
                Dimension::Angle {
                    vertex: t.apply(*vertex),
                    start: t.apply(*a),
                    end: t.apply(*b),
                    value: *value,
                }
            }
        })
    }
}

// ---------------------------------------------------------------------------------------------
// What a click picks

/// A circle or arc picked for a radius or diameter: its centre and radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PickedCircle {
    pub center: Point,
    pub radius: f64,
    /// Distance from the click to the circle or arc itself.
    pub distance: f64,
}

/// A straight line picked for an angle: its two ends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PickedLine {
    pub a: Point,
    pub b: Point,
    /// Distance from the click to the line itself.
    pub distance: f64,
}

/// The circle, or the arc nearest to `click`, of a shape: a circle, an arc, or an arc segment
/// of a polyline.
pub fn pick_circle(shape: &Shape, click: Point) -> Option<PickedCircle> {
    match shape {
        Shape::Circle { center, radius } => Some(PickedCircle {
            center: *center,
            radius: *radius,
            distance: (click.distance_to(*center) - radius).abs(),
        }),
        Shape::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let s = on_circle(*center, *radius, start_angle.to_radians());
            let e = on_circle(*center, *radius, end_angle.to_radians());
            let sweep = crate::geometry::arc_sweep_deg(*start_angle, *end_angle).to_radians();
            let bulge = crate::geometry::bulge_from_sweep(sweep);
            Some(PickedCircle {
                center: *center,
                radius: *radius,
                distance: distance_to_segment(click, s, e, bulge),
            })
        }
        Shape::Polyline(pl) => segments(&pl.vertices, pl.closed)
            .filter_map(|(a, b, bulge)| {
                let arc = bulge_arc(a, b, bulge)?;
                Some(PickedCircle {
                    center: arc.center,
                    radius: arc.radius,
                    distance: distance_to_segment(click, a, b, bulge),
                })
            })
            .min_by(|x, y| x.distance.total_cmp(&y.distance)),
        _ => None,
    }
}

/// The line, or the straight segment nearest to `click`, of a shape: a line, a straight
/// segment of a polyline, or a side of a rectangle.
pub fn pick_line(shape: &Shape, click: Point) -> Option<PickedLine> {
    let nearest = |segs: Vec<(Point, Point)>| {
        segs.into_iter()
            .filter(|(a, b)| a.distance_to(*b) > SAME_POINT)
            .map(|(a, b)| PickedLine {
                a,
                b,
                distance: distance_to_segment(click, a, b, 0.0),
            })
            .min_by(|x, y| x.distance.total_cmp(&y.distance))
    };
    match shape {
        Shape::Line { start, end } => nearest(vec![(*start, *end)]),
        Shape::Polyline(pl) => nearest(
            segments(&pl.vertices, pl.closed)
                .filter(|s| s.2 == 0.0)
                .map(|(a, b, _)| (a, b))
                .collect(),
        ),
        Shape::Rectangle {
            lower_left,
            width,
            height,
            rotation,
        } => {
            let c = rectangle_corners(*lower_left, *width, *height, *rotation);
            nearest((0..4).map(|i| (c[i], c[(i + 1) % 4])).collect())
        }
        _ => None,
    }
}

fn segments(
    v: &[crate::geometry::Vertex],
    closed: bool,
) -> impl Iterator<Item = (Point, Point, f64)> + '_ {
    let n = v.len();
    let count = if closed { n } else { n.saturating_sub(1) };
    (0..count).map(move |i| (v[i].point(), v[(i + 1) % n].point(), v[i].bulge))
}

// ---------------------------------------------------------------------------------------------
// Building from clicks

/// An aligned dimension from `from` to `to` with its line through `at`.
pub fn aligned(from: Point, to: Point, at: Point) -> Option<Dimension> {
    let value = from.distance_to(to);
    (value > SAME_POINT).then_some(Dimension::Aligned {
        from,
        to,
        at,
        value,
    })
}

/// A radius (or, with `diameter`, a diameter) dimension of `circle`, written at `at`.
pub fn circular(circle: PickedCircle, at: Point, diameter: bool) -> Option<Dimension> {
    let (center, radius) = (circle.center, circle.radius);
    if radius.is_nan() || radius <= SAME_POINT {
        return None;
    }
    Some(if diameter {
        Dimension::Diameter {
            center,
            radius,
            at,
            value: 2.0 * radius,
        }
    } else {
        Dimension::Radius {
            center,
            radius,
            at,
            value: radius,
        }
    })
}

/// The angle between two lines that contains the direction of `at` from where the lines cross,
/// with its arc through `at`. `None` if the lines are parallel or `at` is on the crossing.
pub fn angle(l1: PickedLine, l2: PickedLine, at: Point) -> Option<Dimension> {
    let d1 = sub(l1.b, l1.a);
    let d2 = sub(l2.b, l2.a);
    let denom = crate::geometry::cross(d1, d2);
    let scale = (d1.x.hypot(d1.y)) * (d2.x.hypot(d2.y));
    if denom.abs() <= 1e-12 * scale {
        return None; // parallel
    }
    // Where the (extended) lines cross.
    let t = crate::geometry::cross(sub(l2.a, l1.a), d2) / denom;
    let vertex = Point::new(l1.a.x + d1.x * t, l1.a.y + d1.y * t);
    let radius = vertex.distance_to(at);
    if radius <= SAME_POINT {
        return None;
    }
    let dir = |d: Point| d.y.atan2(d.x).rem_euclid(TAU);
    let a1 = dir(d1);
    let a2 = dir(d2);
    // The four rays out of the crossing, sorted anticlockwise; the click falls between two
    // neighbours, which bound the angle it chose.
    let mut rays = [a1, (a1 + PI) % TAU, a2, (a2 + PI) % TAU];
    rays.sort_by(f64::total_cmp);
    let q = dir(sub(at, vertex));
    let i = rays.iter().rposition(|r| *r <= q).unwrap_or(3);
    let (start, end) = (rays[i], rays[(i + 1) % 4]);
    let sweep = (end - start).rem_euclid(TAU);
    Some(Dimension::Angle {
        vertex,
        start: on_circle(vertex, radius, start),
        end: on_circle(vertex, radius, start + sweep),
        value: sweep.to_degrees(),
    })
}

// ---------------------------------------------------------------------------------------------
// Laying out for the screen

/// A dimension's value as placed on the drawing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Label {
    pub text: String,
    /// Centre of the text.
    pub at: [f64; 2],
    /// Direction of the text's baseline, degrees anticlockwise from +X (always readable: from
    /// −90° up to 90°).
    pub angle: f64,
    /// Text height in mm.
    pub height: f64,
}

impl Label {
    /// The corners of the text's box (estimated from the number of characters).
    pub fn corners(&self) -> [[f64; 2]; 4] {
        let w = self.text.chars().count() as f64 * CHAR_WIDTH * self.height / 2.0;
        let h = self.height / 2.0;
        let (s, c) = self.angle.to_radians().sin_cos();
        let at = self.at;
        [(-w, -h), (w, -h), (w, h), (-w, h)]
            .map(|(x, y)| [at[0] + x * c - y * s, at[1] + x * s + y * c])
    }

    fn half_width(&self) -> f64 {
        self.text.chars().count() as f64 * CHAR_WIDTH * self.height / 2.0
    }
}

/// How a dimension is drawn: open polylines (dimension and extension lines, arcs, and
/// arrowheads as closed triangles) and its label.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub lines: Vec<Vec<[f64; 2]>>,
    pub label: Label,
}

fn xy(p: Point) -> [f64; 2] {
    [p.x, p.y]
}

fn plus(p: Point, d: Point, k: f64) -> Point {
    Point::new(p.x + d.x * k, p.y + d.y * k)
}

fn unit(d: Point) -> Point {
    let len = d.x.hypot(d.y);
    if len > 0.0 {
        Point::new(d.x / len, d.y / len)
    } else {
        Point::new(1.0, 0.0)
    }
}

/// An arrowhead with its tip at `tip`, pointing along `towards` (a unit direction).
fn arrow(tip: Point, towards: Point) -> Vec<[f64; 2]> {
    let back = plus(tip, towards, -ARROW_LENGTH);
    let n = Point::new(-towards.y, towards.x);
    vec![
        xy(tip),
        xy(plus(back, n, ARROW_HALF_WIDTH)),
        xy(plus(back, n, -ARROW_HALF_WIDTH)),
        xy(tip),
    ]
}

/// A readable text direction (degrees, from −90 up to 90) for a line running at `deg`.
fn readable(deg: f64) -> f64 {
    let a = (deg + 180.0).rem_euclid(360.0) - 180.0; // −180..180
    if a > 90.0 {
        a - 180.0
    } else if a <= -90.0 {
        a + 180.0
    } else {
        a
    }
}

/// A label beside `p`, on the far side along the unit direction `out`, horizontal.
fn label_beyond(text: String, p: Point, out: Point) -> Label {
    let mut label = Label {
        text,
        at: xy(p),
        angle: 0.0,
        height: TEXT_HEIGHT,
    };
    // Push the box out along `out` until its nearest edge clears `p` by the gap.
    let reach = label.half_width() * out.x.abs() + TEXT_HEIGHT / 2.0 * out.y.abs();
    label.at = xy(plus(p, out, reach + LABEL_GAP));
    label
}

pub fn layout(d: &Dimension) -> Layout {
    let text = d.text();
    match *d {
        Dimension::Aligned { from, to, at, .. } => {
            let u = unit(sub(to, from));
            let n = Point::new(-u.y, u.x);
            let offset = dot(sub(at, from), n);
            let side = if offset < 0.0 { -1.0 } else { 1.0 };
            let (a, b) = (plus(from, n, offset), plus(to, n, offset));
            let mut lines = Vec::new();
            // Extension lines from just off each point to just past the dimension line.
            if offset.abs() > EXTENSION_GAP {
                for p in [from, to] {
                    lines.push(vec![
                        xy(plus(p, n, side * EXTENSION_GAP)),
                        xy(plus(p, n, offset + side * EXTENSION_BEYOND)),
                    ]);
                }
            }
            lines.push(vec![xy(a), xy(b)]);
            lines.push(arrow(a, Point::new(-u.x, -u.y)));
            lines.push(arrow(b, u));
            let mid = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
            Layout {
                lines,
                label: Label {
                    text,
                    at: xy(plus(mid, n, side * (LABEL_GAP + TEXT_HEIGHT / 2.0))),
                    angle: readable(u.y.atan2(u.x).to_degrees()),
                    height: TEXT_HEIGHT,
                },
            }
        }
        Dimension::Radius {
            center, radius, at, ..
        } => {
            let w = unit(sub(at, center));
            let edge = plus(center, w, radius);
            // Leader from the centre to the arc, and on to the value if it is outside.
            let far = if center.distance_to(at) > radius {
                at
            } else {
                edge
            };
            Layout {
                lines: vec![vec![xy(center), xy(far)], arrow(edge, w)],
                label: label_beyond(text, far, w),
            }
        }
        Dimension::Diameter {
            center, radius, at, ..
        } => {
            let w = unit(sub(at, center));
            let edge = plus(center, w, radius);
            let back = plus(center, w, -radius);
            let far = if center.distance_to(at) > radius {
                at
            } else {
                edge
            };
            Layout {
                lines: vec![
                    vec![xy(back), xy(far)],
                    arrow(edge, w),
                    arrow(back, Point::new(-w.x, -w.y)),
                ],
                label: label_beyond(text, far, w),
            }
        }
        Dimension::Angle {
            vertex, start, end, ..
        } => {
            let radius = vertex.distance_to(start);
            let a0 = angle_deg(vertex, start).to_radians();
            let sweep = (angle_deg(vertex, end).to_radians() - a0).rem_euclid(TAU);
            let steps = ((sweep.to_degrees() / 3.0).ceil() as usize).max(2);
            let arc: Vec<[f64; 2]> = (0..=steps)
                .map(|i| {
                    xy(on_circle(
                        vertex,
                        radius,
                        a0 + sweep * i as f64 / steps as f64,
                    ))
                })
                .collect();
            // Arrow tips at the arc's ends, pointing outwards along the arc.
            let tangent = |a: f64| Point::new(-a.sin(), a.cos());
            let t0 = tangent(a0);
            let t1 = tangent(a0 + sweep);
            let mid_dir = Point::new((a0 + sweep / 2.0).cos(), (a0 + sweep / 2.0).sin());
            Layout {
                lines: vec![
                    vec![xy(vertex), xy(start)],
                    vec![xy(vertex), xy(end)],
                    arc,
                    arrow(start, Point::new(-t0.x, -t0.y)),
                    arrow(end, t1),
                ],
                label: label_beyond(text, plus(vertex, mid_dir, radius), mid_dir),
            }
        }
    }
}
