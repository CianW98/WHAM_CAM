//! Turns the document into simple point lists the interface can draw.
//! Arcs, circles, splines and ellipses are approximated with short straight segments for the
//! screen only; the drawing itself (and later the toolpaths) keep the exact curves.

use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

use crate::document::{Document, LAYER_PALETTE};
use crate::geometry::{Point, Polyline, Shape, Spline, rectangle_corners};

/// Maximum distance between a true arc and its on-screen approximation, in mm.
pub const DISPLAY_TOLERANCE: f64 = 0.01;
/// Line segments per knot span when drawing a spline.
const SPLINE_STEPS_PER_SPAN: usize = 24;
/// Most segments used for any one curve, so huge curves can't stall the screen.
const MAX_CURVE_STEPS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayData {
    pub shapes: Vec<DisplayShape>,
    /// [min_x, min_y, max_x, max_y] of everything, or `None` if the drawing is empty.
    pub bounds: Option<[f64; 4]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayShape {
    /// The drawing object this came from.
    pub id: u32,
    /// Points in order, as [x, y] pairs, in mm.
    pub points: Vec<[f64; 2]>,
    /// True if the last point joins back to the first.
    pub closed: bool,
    /// True if the points are separate markers (a point list), not a connected path.
    pub markers: bool,
    /// Colour to draw it in ("#rrggbb"): its layer's colour.
    pub color: String,
    /// Which way a closed polyline or rectangle runs; `None` for other shapes.
    pub direction: Option<Direction>,
}

/// Which way a closed shape runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// Clockwise.
    CW,
    /// Counter-clockwise.
    CCW,
}

/// Signed area of a closed polyline (positive = counter-clockwise), counting bulge arcs exactly.
pub fn polyline_signed_area(pl: &Polyline) -> f64 {
    let n = pl.vertices.len();
    let mut area = 0.0;
    for i in 0..n {
        let (a, b) = (pl.vertices[i], pl.vertices[(i + 1) % n]);
        area += (a.x * b.y - b.x * a.y) / 2.0;
        if a.bulge != 0.0 {
            // The segment between the chord and the arc: r²/2 · (θ − sin θ), signed with θ.
            let theta = 4.0 * a.bulge.atan();
            let half_chord = a.point().distance_to(b.point()) / 2.0;
            let r = half_chord / (theta / 2.0).sin().abs();
            area += r * r / 2.0 * (theta - theta.sin());
        }
    }
    area
}

pub fn display_data(doc: &Document) -> DisplayData {
    let shapes: Vec<DisplayShape> = doc
        .layers
        .iter()
        .flat_map(|layer| {
            layer.objects.iter().map(|obj| DisplayShape {
                color: layer.color.clone(),
                ..display_shape(obj.id, &obj.shape)
            })
        })
        .collect();
    let mut bounds: Option<[f64; 4]> = None;
    for p in shapes.iter().flat_map(|s| s.points.iter()) {
        let b = bounds.get_or_insert([p[0], p[1], p[0], p[1]]);
        b[0] = b[0].min(p[0]);
        b[1] = b[1].min(p[1]);
        b[2] = b[2].max(p[0]);
        b[3] = b[3].max(p[1]);
    }
    DisplayData { shapes, bounds }
}

pub fn display_shape(id: u32, shape: &Shape) -> DisplayShape {
    let mut closed = false;
    let mut markers = false;
    let mut direction = None;
    let points = match shape {
        Shape::Line { start, end } => vec![[start.x, start.y], [end.x, end.y]],
        Shape::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let mut pts = Vec::new();
            let sweep = end_angle - start_angle;
            push_arc(
                &mut pts,
                *center,
                *radius,
                start_angle.to_radians(),
                sweep.to_radians(),
            );
            pts
        }
        Shape::Circle { center, radius } => {
            let mut pts = Vec::new();
            push_arc(&mut pts, *center, *radius, 0.0, TAU);
            pts.pop(); // last point duplicates the first; the shape is closed
            closed = true;
            pts
        }
        Shape::Polyline(pl) => {
            closed = pl.closed;
            if pl.closed && pl.vertices.len() >= 2 {
                direction = Some(if polyline_signed_area(pl) < 0.0 {
                    Direction::CW
                } else {
                    Direction::CCW
                });
            }
            polyline_points(pl)
        }
        Shape::Rectangle {
            lower_left,
            width,
            height,
            rotation,
        } => {
            closed = true;
            direction = Some(Direction::CCW);
            rectangle_corners(*lower_left, *width, *height, *rotation)
                .iter()
                .map(|p| [p.x, p.y])
                .collect()
        }
        Shape::Points { points } => {
            markers = true;
            points.iter().map(|p| [p.x, p.y]).collect()
        }
        Shape::Spline(s) => spline_points(s),
        Shape::Ellipse {
            center,
            major_axis,
            ratio,
            start_param,
            end_param,
        } => ellipse_points(*center, *major_axis, *ratio, *start_param, *end_param),
    };
    DisplayShape {
        id,
        points,
        closed,
        markers,
        color: LAYER_PALETTE[0].to_string(),
        direction,
    }
}

fn polyline_points(pl: &Polyline) -> Vec<[f64; 2]> {
    let n = pl.vertices.len();
    let mut pts = Vec::new();
    if n == 0 {
        return pts;
    }
    let segments = if pl.closed { n } else { n - 1 };
    pts.push([pl.vertices[0].x, pl.vertices[0].y]);
    for i in 0..segments {
        let a = pl.vertices[i];
        let b = pl.vertices[(i + 1) % n];
        if a.bulge != 0.0
            && let Some((center, radius, start, sweep)) = bulge_arc(a.point(), b.point(), a.bulge)
        {
            push_arc(&mut pts, center, radius, start, sweep);
            pts.pop(); // replaced by the exact end vertex below
        }
        pts.push([b.x, b.y]);
    }
    if pl.closed {
        pts.pop(); // the closing point duplicates the first vertex
    }
    pts
}

/// Centre, radius, start angle and signed sweep of the arc from `a` to `b` with the given bulge.
pub(crate) fn bulge_arc(a: Point, b: Point, bulge: f64) -> Option<(Point, f64, f64, f64)> {
    let chord = a.distance_to(b);
    if chord == 0.0 {
        return None;
    }
    let sweep = 4.0 * bulge.atan();
    // Distance from chord midpoint to centre, towards the left of a→b.
    let offset = chord / 2.0 * (1.0 - bulge * bulge) / (2.0 * bulge);
    let (dx, dy) = ((b.x - a.x) / chord, (b.y - a.y) / chord);
    let center = Point::new(
        (a.x + b.x) / 2.0 - dy * offset,
        (a.y + b.y) / 2.0 + dx * offset,
    );
    let radius = center.distance_to(a);
    let start = (a.y - center.y).atan2(a.x - center.x);
    Some((center, radius, start, sweep))
}

/// Appends points along an arc (angles in radians), including both the start and end points.
fn push_arc(pts: &mut Vec<[f64; 2]>, center: Point, radius: f64, start: f64, sweep: f64) {
    let max_step = if radius > DISPLAY_TOLERANCE {
        2.0 * (1.0 - DISPLAY_TOLERANCE / radius).acos()
    } else {
        TAU / 8.0
    };
    let steps = ((sweep.abs() / max_step).ceil() as usize).clamp(2, MAX_CURVE_STEPS);
    for i in 0..=steps {
        let angle = start + sweep * (i as f64) / (steps as f64);
        pts.push([
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        ]);
    }
}

pub(crate) fn ellipse_point(center: Point, major: Point, ratio: f64, t: f64) -> [f64; 2] {
    let minor = Point::new(-major.y * ratio, major.x * ratio);
    let (s, c) = t.sin_cos();
    [
        center.x + major.x * c + minor.x * s,
        center.y + major.y * c + minor.y * s,
    ]
}

fn ellipse_points(center: Point, major: Point, ratio: f64, start: f64, end: f64) -> Vec<[f64; 2]> {
    // Step size from the larger radius, as for a circle of that radius.
    let r = major.x.hypot(major.y);
    let max_step = if r > DISPLAY_TOLERANCE {
        2.0 * (1.0 - DISPLAY_TOLERANCE / r).acos()
    } else {
        TAU / 8.0
    };
    let sweep = end - start;
    let steps = ((sweep.abs() / max_step).ceil() as usize).clamp(2, MAX_CURVE_STEPS);
    (0..=steps)
        .map(|i| {
            ellipse_point(
                center,
                major,
                ratio,
                start + sweep * i as f64 / steps as f64,
            )
        })
        .collect()
}

/// Point on a (possibly rational) B-spline at parameter `u`, by de Boor's algorithm.
pub(crate) fn spline_point(s: &Spline, u: f64) -> [f64; 2] {
    let p = s.degree;
    let n = s.control_points.len();
    let k = &s.knots;
    // Knot span index: largest k_index with knots[k_index] <= u, limited to the valid range.
    let mut span = p;
    while span + 1 < n && k[span + 1] <= u {
        span += 1;
    }
    let w = |i: usize| s.weights.get(i).copied().unwrap_or(1.0);
    // Homogeneous control points (x·w, y·w, w).
    let mut d: Vec<[f64; 3]> = (0..=p)
        .map(|j| {
            let i = span + j - p;
            let cp = s.control_points[i];
            [cp.x * w(i), cp.y * w(i), w(i)]
        })
        .collect();
    for r in 1..=p {
        for j in (r..=p).rev() {
            let i = span + j - p;
            let denom = k[i + p + 1 - r] - k[i];
            let alpha = if denom == 0.0 {
                0.0
            } else {
                (u - k[i]) / denom
            };
            let prev = d[j - 1];
            for (c, v) in d[j].iter_mut().enumerate() {
                *v = (1.0 - alpha) * prev[c] + alpha * *v;
            }
        }
    }
    let [x, y, wt] = d[p];
    [x / wt, y / wt]
}

fn spline_points(s: &Spline) -> Vec<[f64; 2]> {
    let p = s.degree;
    let n = s.control_points.len();
    if n <= p || s.knots.len() != n + p + 1 {
        return s.control_points.iter().map(|c| [c.x, c.y]).collect();
    }
    let (u0, u1) = (s.knots[p], s.knots[n]);
    let spans = s.knots[p..=n]
        .windows(2)
        .filter(|w| w[1] > w[0])
        .count()
        .max(1);
    let steps = (spans * SPLINE_STEPS_PER_SPAN).min(MAX_CURVE_STEPS);
    (0..=steps)
        .map(|i| spline_point(s, u0 + (u1 - u0) * i as f64 / steps as f64))
        .collect()
}
