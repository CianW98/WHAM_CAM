//! Splines and ellipses, as read from a DXF, and their conversion into polylines of arcs and
//! lines within a tolerance (decided 2026-11-20: curves are converted on import; the drawing only
//! holds lines, arcs, circles and polylines, so everything imported can be machined).
//!
//! The conversion fits **biarcs**: pairs of arcs that meet smoothly and follow the curve's
//! direction at both ends. Each piece of the curve gets one biarc; if the biarc strays from the
//! curve by more than the tolerance anywhere, the piece is split in two and each half is fitted
//! again. The result is smooth (no faceting) and much shorter than a list of short lines.

use std::f64::consts::{FRAC_PI_2, TAU};

use crate::geometry::{
    EllipticalArc, Point, Polyline, Transform, Vertex, bulge_from_sweep, cross,
    distance_to_segment, dot, mid, point_along_segment, sub,
};

/// A NURBS curve as stored in a DXF SPLINE.
#[derive(Debug, Clone, PartialEq)]
pub struct Spline {
    pub degree: usize,
    pub control_points: Vec<Point>,
    pub knots: Vec<f64>,
    /// One weight per control point. Empty means all weights are 1 (a non-rational spline).
    pub weights: Vec<f64>,
    pub closed: bool,
}

/// A curve that is converted into a polyline on import.
#[derive(Debug, Clone, PartialEq)]
pub enum Curve {
    Spline(Spline),
    /// Point at parameter t = center + major_axis·cos t + minor_axis·sin t, where minor_axis is
    /// major_axis turned 90° counter-clockwise and scaled by `ratio`. Runs counter-clockwise from
    /// `start_param` to `end_param` (radians; 0 to 2π for a full ellipse).
    Ellipse {
        center: Point,
        major_axis: Point,
        ratio: f64,
        start_param: f64,
        end_param: f64,
    },
}

/// How many times a piece may be halved before it is drawn as a straight line instead.
const MAX_SPLITS: u32 = 30;
/// Points checked along each piece (curve to biarc), and along each arc (biarc to curve).
const CHECKS: usize = 16;

impl Curve {
    pub fn kind(&self) -> &'static str {
        match self {
            Curve::Spline(_) => "SPLINE",
            Curve::Ellipse { .. } => "ELLIPSE",
        }
    }

    /// The transformed curve, or `None` if the result isn't the same kind of curve (an ellipse
    /// under an uneven scale). Splines follow any transform exactly through their control points.
    pub fn transformed(&self, t: &Transform) -> Option<Curve> {
        Some(match self {
            Curve::Spline(s) => Curve::Spline(Spline {
                control_points: s.control_points.iter().map(|p| t.apply(*p)).collect(),
                ..s.clone()
            }),
            Curve::Ellipse {
                center,
                major_axis,
                ratio,
                start_param,
                end_param,
            } => {
                t.uniform_scale()?;
                let (start, end) = if t.mirrors() {
                    // Mirroring runs the ellipse the other way: parameter t becomes -t.
                    (-end_param, -start_param)
                } else {
                    (*start_param, *end_param)
                };
                let shift = if start < 0.0 { TAU } else { 0.0 };
                Curve::Ellipse {
                    center: t.apply(*center),
                    major_axis: t.apply_vector(*major_axis),
                    ratio: *ratio,
                    start_param: start + shift,
                    end_param: end + shift,
                }
            }
        })
    }

    /// The point at parameter `u`.
    pub fn point(&self, u: f64) -> Point {
        match self {
            Curve::Spline(s) => spline_point(s, u),
            Curve::Ellipse {
                center,
                major_axis,
                ratio,
                ..
            } => {
                let minor = Point::new(-major_axis.y * ratio, major_axis.x * ratio);
                let (sn, cs) = u.sin_cos();
                Point::new(
                    center.x + major_axis.x * cs + minor.x * sn,
                    center.y + major_axis.y * cs + minor.y * sn,
                )
            }
        }
    }

    /// Parameter ranges on which the curve is smooth, in order: a spline's knot spans (it may
    /// have a corner where two spans meet), an ellipse in pieces of at most 90°.
    fn pieces(&self) -> Vec<(f64, f64)> {
        match self {
            Curve::Spline(s) => {
                let (p, n) = (s.degree, s.control_points.len());
                let mut knots: Vec<f64> = s.knots[p..=n].to_vec();
                knots.dedup();
                knots.windows(2).map(|w| (w[0], w[1])).collect()
            }
            Curve::Ellipse {
                start_param,
                end_param,
                ..
            } => {
                let sweep = end_param - start_param;
                let count = (sweep / FRAC_PI_2 - 1e-9).ceil().max(1.0) as usize;
                (0..count)
                    .map(|i| {
                        let at = |k: usize| start_param + sweep * k as f64 / count as f64;
                        (at(i), at(i + 1))
                    })
                    .collect()
            }
        }
    }

    fn is_closed(&self) -> bool {
        match self {
            Curve::Spline(s) => s.closed,
            Curve::Ellipse {
                start_param,
                end_param,
                ..
            } => end_param - start_param >= TAU - 1e-9,
        }
    }

    /// The curve as a polyline of arcs and lines that stays within `tolerance` of it (both ways:
    /// every point of the curve is near the polyline and every point of the polyline is near the
    /// curve). `None` if the curve can't be read (bad spline data, no length).
    pub fn to_polyline(&self, tolerance: f64) -> Option<Polyline> {
        if let Curve::Spline(s) = self {
            let ok = s.degree >= 1
                && s.control_points.len() > s.degree
                && s.knots.len() == s.control_points.len() + s.degree + 1
                && (s.weights.is_empty() || s.weights.len() == s.control_points.len())
                && s.knots.windows(2).all(|w| w[1] >= w[0]);
            if !ok {
                return None;
            }
        }
        let pieces = self.pieces();
        let first = pieces.first()?;
        let start = self.point(first.0);
        let mut vertices = vec![Vertex::line(start.x, start.y)];
        for &(a, b) in &pieces {
            self.fit(a, b, tolerance, 0, &mut vertices);
        }
        // Drop zero-length steps (e.g. a spline span that doesn't move).
        vertices.dedup_by(|b, a| a.point().distance_to(b.point()) < 1e-9);
        let start = vertices[0].point();
        let end = vertices.last()?.point();
        let closed = self.is_closed() || (vertices.len() > 2 && start.distance_to(end) < 1e-9);
        if closed && vertices.len() > 2 && start.distance_to(end) < 1e-6 {
            vertices.pop();
        }
        if vertices.len() < 2 {
            return None;
        }
        Some(Polyline { vertices, closed })
    }

    /// Fits the piece from `a` to `b` (its start is already the last vertex), appending the end
    /// vertex of each arc or line.
    fn fit(&self, a: f64, b: f64, tolerance: f64, splits: u32, out: &mut Vec<Vertex>) {
        let p1 = self.point(b);
        if splits < MAX_SPLITS
            && let Some(segments) = self.biarc(a, b)
            && self.within(a, b, &segments, tolerance)
        {
            for (bulge, end) in segments {
                out.last_mut().expect("starts with a vertex").bulge = bulge;
                out.push(Vertex::line(end.x, end.y));
            }
            return;
        }
        if splits >= MAX_SPLITS {
            // Tiny and still not fitting (e.g. a cusp): a straight line is within the tolerance.
            out.push(Vertex::line(p1.x, p1.y));
            return;
        }
        let m = (a + b) / 2.0;
        self.fit(a, m, tolerance, splits + 1, out);
        self.fit(m, b, tolerance, splits + 1, out);
    }

    /// Unit direction of travel at `u`, measured inside the piece `a..b` (so a corner where two
    /// pieces meet doesn't blur the direction).
    fn direction(&self, u: f64, a: f64, b: f64) -> Option<Point> {
        let h = (b - a) * 1e-6;
        let (u0, u1) = if u - h < a {
            (u, u + h)
        } else if u + h > b {
            (u - h, u)
        } else {
            (u - h, u + h)
        };
        let d = sub(self.point(u1), self.point(u0));
        let len = d.x.hypot(d.y);
        (len > 0.0 && len.is_finite()).then(|| Point::new(d.x / len, d.y / len))
    }

    /// The biarc from `a` to `b`: up to two (bulge, end point) segments, following the curve's
    /// direction at both ends. `None` if the ends' directions don't allow one.
    fn biarc(&self, a: f64, b: f64) -> Option<Vec<(f64, Point)>> {
        let (p0, p1) = (self.point(a), self.point(b));
        let (t0, t1) = (self.direction(a, a, b)?, self.direction(b, a, b)?);
        let v = sub(p1, p0);
        let vv = dot(v, v);
        if vv == 0.0 {
            return None;
        }
        // Equal tangent lengths d from each end: |(p0 + d·t0) − (p1 − d·t1)| = 2d.
        let denom = 2.0 * (1.0 - dot(t0, t1));
        let vt = dot(v, Point::new(t0.x + t1.x, t0.y + t1.y));
        let d = if denom < 1e-12 {
            // Parallel end directions: a straight line if the chord runs that way too.
            if cross(t0, v).abs() < 1e-12 * vv.sqrt() && dot(t0, v) > 0.0 {
                return Some(vec![(0.0, p1)]);
            }
            let vt1 = dot(v, t1);
            if vt1 <= 0.0 {
                return None;
            }
            vv / (4.0 * vt1)
        } else {
            (-vt + (vt * vt + denom * vv).sqrt()) / denom
        };
        if !(d > 0.0 && d.is_finite()) {
            return None;
        }
        let join = mid(
            Point::new(p0.x + d * t0.x, p0.y + d * t0.y),
            Point::new(p1.x - d * t1.x, p1.y - d * t1.y),
        );
        // Each arc turns through twice the angle between its end direction and its chord.
        let c1 = sub(join, p0);
        let c2 = sub(p1, join);
        let bulge1 = bulge_from_sweep(2.0 * cross(t0, c1).atan2(dot(t0, c1)));
        let bulge2 = bulge_from_sweep(2.0 * cross(c2, t1).atan2(dot(c2, t1)));
        let straight = |b: f64| if b.abs() < 1e-12 { 0.0 } else { b };
        Some(vec![(straight(bulge1), join), (straight(bulge2), p1)])
    }

    /// Whether `segments` (starting at the point at `a`) stay within `tolerance` of the curve
    /// from `a` to `b`, checked both ways.
    fn within(&self, a: f64, b: f64, segments: &[(f64, Point)], tolerance: f64) -> bool {
        // The curve, finely sampled.
        let samples: Vec<Point> = (0..=CHECKS * 2)
            .map(|i| self.point(a + (b - a) * i as f64 / (CHECKS * 2) as f64))
            .collect();
        let mut starts = vec![self.point(a)];
        starts.extend(segments.iter().map(|s| s.1));
        let pieces: Vec<(Point, Point, f64)> = segments
            .iter()
            .enumerate()
            .map(|(i, &(bulge, end))| (starts[i], end, bulge))
            .collect();
        // Curve → biarc.
        let curve_ok = samples.iter().all(|&p| {
            pieces
                .iter()
                .map(|&(s, e, bulge)| distance_to_segment(p, s, e, bulge))
                .fold(f64::INFINITY, f64::min)
                <= tolerance
        });
        // Biarc → curve (against the fine samples, joined by straight lines).
        curve_ok
            && pieces.iter().all(|&(s, e, bulge)| {
                (1..CHECKS).all(|i| {
                    let p = point_along_segment(s, e, bulge, i as f64 / CHECKS as f64);
                    samples
                        .windows(2)
                        .map(|w| distance_to_segment(p, w[0], w[1], 0.0))
                        .fold(f64::INFINITY, f64::min)
                        <= tolerance
                })
            })
    }
}

/// A cubic Bézier curve as a spline (exact).
pub fn cubic_bezier(p0: Point, p1: Point, p2: Point, p3: Point) -> Spline {
    Spline {
        degree: 3,
        control_points: vec![p0, p1, p2, p3],
        knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        weights: Vec::new(),
        closed: false,
    }
}

/// A quadratic Bézier curve as a spline (exact).
pub fn quadratic_bezier(p0: Point, p1: Point, p2: Point) -> Spline {
    Spline {
        degree: 2,
        control_points: vec![p0, p1, p2],
        knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        weights: Vec::new(),
        closed: false,
    }
}

/// An elliptical arc as a rational quadratic spline (exact): pieces of at most 90°, each
/// weighted so it lies exactly on the ellipse. Being a spline, it stays exact under any
/// transform of its control points (e.g. an SVG skew).
pub fn elliptical_arc(arc: &EllipticalArc) -> Spline {
    let n = (arc.sweep.abs() / FRAC_PI_2 - 1e-9).ceil().max(1.0) as usize;
    let step = arc.sweep / n as f64;
    let (sp, cp) = arc.phi.sin_cos();
    // From the unit circle to the ellipse.
    let place = |x: f64, y: f64| {
        let (ex, ey) = (arc.rx * x, arc.ry * y);
        Point::new(
            arc.center.x + cp * ex - sp * ey,
            arc.center.y + sp * ex + cp * ey,
        )
    };
    let w = (step / 2.0).cos();
    let mut control_points = vec![place(arc.start.cos(), arc.start.sin())];
    let mut weights = vec![1.0];
    let mut knots = vec![0.0; 3];
    for i in 0..n {
        let a = arc.start + step * i as f64;
        let m = a + step / 2.0;
        control_points.push(place(m.cos() / w, m.sin() / w));
        weights.push(w);
        control_points.push(place((a + step).cos(), (a + step).sin()));
        weights.push(1.0);
        let k = (i + 1) as f64;
        knots.extend(if i + 1 < n { vec![k, k] } else { vec![k; 3] });
    }
    Spline {
        degree: 2,
        control_points,
        knots,
        weights,
        closed: false,
    }
}

/// Point on a (possibly rational) B-spline at parameter `u`, by de Boor's algorithm.
pub fn spline_point(s: &Spline, u: f64) -> Point {
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
    Point::new(x / wt, y / wt)
}
