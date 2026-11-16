//! Edit tools: join lines and arcs into polylines, explode shapes into lines and arcs, close
//! polylines, and break shapes where they cross.

use std::f64::consts::TAU;

use crate::display::bulge_arc;
use crate::geometry::{Point, Polyline, Shape, Vertex, rectangle_corners};

/// Points closer than this (mm) count as the same point when closing or breaking.
pub const EDIT_TOLERANCE: f64 = 0.001;

// --- Small helpers ------------------------------------------------------------------------------

fn sub(a: Point, b: Point) -> Point {
    Point::new(a.x - b.x, a.y - b.y)
}

fn cross(a: Point, b: Point) -> f64 {
    a.x * b.y - a.y * b.x
}

fn dot(a: Point, b: Point) -> f64 {
    a.x * b.x + a.y * b.y
}

/// An angle in degrees in [0, 360), with values a hair under 360 taken as 0.
fn norm_deg(a: f64) -> f64 {
    let a = a.rem_euclid(360.0);
    if 360.0 - a < 1e-9 { 0.0 } else { a }
}

/// The sweep of a stored (counter-clockwise) arc in degrees, in (0, 360].
fn arc_sweep_deg(start: f64, end: f64) -> f64 {
    let s = (end - start).rem_euclid(360.0);
    if s < 1e-9 { 360.0 } else { s }
}

fn on_circle(c: Point, r: f64, angle_rad: f64) -> Point {
    Point::new(c.x + r * angle_rad.cos(), c.y + r * angle_rad.sin())
}

// --- Simple pieces: a straight segment or an arc ----------------------------------------------

/// One straight segment or arc. Arcs run from `start` (radians) through `sweep` (radians,
/// positive = counter-clockwise).
#[derive(Debug, Clone, Copy)]
enum Prim {
    Line {
        a: Point,
        b: Point,
    },
    Arc {
        c: Point,
        r: f64,
        start: f64,
        sweep: f64,
    },
}

impl Prim {
    /// The straight segment or arc between two polyline vertices.
    fn from_bulge(a: Point, b: Point, bulge: f64) -> Prim {
        match (bulge != 0.0).then(|| bulge_arc(a, b, bulge)).flatten() {
            Some((c, r, start, sweep)) => Prim::Arc { c, r, start, sweep },
            None => Prim::Line { a, b },
        }
    }

    fn at(&self, t: f64) -> Point {
        match *self {
            Prim::Line { a, b } => Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t),
            Prim::Arc { c, r, start, sweep } => on_circle(c, r, start + sweep * t),
        }
    }

    fn length(&self) -> f64 {
        match *self {
            Prim::Line { a, b } => a.distance_to(b),
            Prim::Arc { r, sweep, .. } => r * sweep.abs(),
        }
    }

    /// The part between parameters t0 and t1 (0 = start, 1 = end).
    fn part(&self, t0: f64, t1: f64) -> Prim {
        match *self {
            Prim::Line { .. } => Prim::Line {
                a: self.at(t0),
                b: self.at(t1),
            },
            Prim::Arc { c, r, start, sweep } => Prim::Arc {
                c,
                r,
                start: start + sweep * t0,
                sweep: sweep * (t1 - t0),
            },
        }
    }

    /// The polyline bulge of this piece (0 for a straight segment).
    fn bulge(&self) -> f64 {
        match *self {
            Prim::Line { .. } => 0.0,
            Prim::Arc { sweep, .. } => (sweep / 4.0).tan(),
        }
    }

    /// As a stand-alone shape: a line, or an arc stored counter-clockwise.
    fn to_shape(self) -> Shape {
        match self {
            Prim::Line { a, b } => Shape::Line { start: a, end: b },
            Prim::Arc { c, r, start, sweep } => {
                let (from, to) = if sweep >= 0.0 {
                    (start, start + sweep)
                } else {
                    (start + sweep, start)
                };
                Shape::Arc {
                    center: c,
                    radius: r,
                    start_angle: norm_deg(from.to_degrees()),
                    end_angle: norm_deg(to.to_degrees()),
                }
            }
        }
    }
}

/// The segments of a polyline, including the closing one if it is closed.
fn polyline_prims(pl: &Polyline) -> Vec<Prim> {
    let v = &pl.vertices;
    let n = v.len();
    let count = if pl.closed { n } else { n.saturating_sub(1) };
    (0..count)
        .map(|i| {
            let (a, b) = (v[i], v[(i + 1) % n]);
            Prim::from_bulge(a.point(), b.point(), a.bulge)
        })
        .collect()
}

// --- Explode ------------------------------------------------------------------------------------

/// Breaks a polyline or rectangle into separate lines and arcs (zero-length segments are
/// dropped). `None` for other shapes, which can't be exploded.
pub fn explode(shape: &Shape) -> Option<Vec<Shape>> {
    let prims = match shape {
        Shape::Polyline(pl) => polyline_prims(pl),
        Shape::Rectangle {
            lower_left,
            width,
            height,
            rotation,
        } => {
            let c = rectangle_corners(*lower_left, *width, *height, *rotation);
            (0..4)
                .map(|i| Prim::Line {
                    a: c[i],
                    b: c[(i + 1) % 4],
                })
                .collect()
        }
        _ => return None,
    };
    Some(
        prims
            .into_iter()
            .filter(|p| p.length() > 1e-9)
            .map(Prim::to_shape)
            .collect(),
    )
}

// --- Close --------------------------------------------------------------------------------------

/// The polyline closed. If its last point sits on its first (within [`EDIT_TOLERANCE`]), the
/// duplicate is dropped. `None` if it is already closed or too short to close.
pub fn close_polyline(pl: &Polyline) -> Option<Polyline> {
    if pl.closed {
        return None;
    }
    let mut vertices = pl.vertices.clone();
    if vertices.len() >= 3 {
        let (first, last) = (vertices[0].point(), vertices[vertices.len() - 1].point());
        if first.distance_to(last) <= EDIT_TOLERANCE {
            vertices.pop();
        }
    }
    // Two points make a shape only if a segment between them is an arc.
    let ok = vertices.len() >= 3
        || (vertices.len() == 2 && (vertices[0].bulge != 0.0 || vertices[1].bulge != 0.0));
    ok.then_some(Polyline {
        vertices,
        closed: true,
    })
}

// --- Join ---------------------------------------------------------------------------------------

/// The result of joining: each chain is a new polyline made from the listed input shapes
/// (indexes into the input, in chain order).
#[derive(Debug, Clone, PartialEq)]
pub struct Joined {
    pub polyline: Polyline,
    pub sources: Vec<usize>,
}

/// An open run of vertices (the last vertex's bulge is unused).
fn as_run(shape: &Shape) -> Option<Vec<Vertex>> {
    match shape {
        Shape::Line { start, end } if start.distance_to(*end) > 0.0 => Some(vec![
            Vertex::line(start.x, start.y),
            Vertex::line(end.x, end.y),
        ]),
        Shape::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let sweep = arc_sweep_deg(*start_angle, *end_angle);
            if sweep >= 360.0 {
                return None; // a full circle has no free ends
            }
            let a = on_circle(*center, *radius, start_angle.to_radians());
            let b = on_circle(*center, *radius, end_angle.to_radians());
            Some(vec![
                Vertex {
                    x: a.x,
                    y: a.y,
                    bulge: (sweep.to_radians() / 4.0).tan(),
                },
                Vertex::line(b.x, b.y),
            ])
        }
        Shape::Polyline(pl) if !pl.closed && pl.vertices.len() >= 2 => {
            let mut v = pl.vertices.clone();
            if let Some(last) = v.last_mut() {
                last.bulge = 0.0;
            }
            Some(v)
        }
        _ => None,
    }
}

/// The same run walked the other way (arcs turn the other way, so bulges change sign).
fn reversed(run: &[Vertex]) -> Vec<Vertex> {
    let n = run.len();
    (0..n)
        .rev()
        .map(|k| Vertex {
            x: run[k].x,
            y: run[k].y,
            bulge: if k > 0 { -run[k - 1].bulge } else { 0.0 },
        })
        .collect()
}

fn first(run: &[Vertex]) -> Point {
    run[0].point()
}

fn last(run: &[Vertex]) -> Point {
    run[run.len() - 1].point()
}

/// Chains lines, arcs and open polylines whose ends are within `tolerance` into polylines.
/// Where several ends are in reach, the closest one is used. A chain whose ends meet is closed.
/// Shapes that can't be joined, and single shapes that touch nothing, are not in the result.
pub fn join(shapes: &[Shape], tolerance: f64) -> Vec<Joined> {
    let runs: Vec<Option<Vec<Vertex>>> = shapes.iter().map(as_run).collect();
    let mut used: Vec<bool> = runs.iter().map(Option::is_none).collect();
    let mut out = Vec::new();

    // The unused run with an end closest to `at` (within the tolerance): its index, and whether
    // the matching end is its start.
    let nearest = |used: &[bool], at: Point| -> Option<(usize, bool)> {
        let mut best: Option<(usize, bool, f64)> = None;
        for (i, run) in runs.iter().enumerate() {
            let Some(run) = run.as_deref().filter(|_| !used[i]) else {
                continue;
            };
            for (at_start, end) in [(true, first(run)), (false, last(run))] {
                let d = end.distance_to(at);
                if d <= tolerance && best.is_none_or(|b| d < b.2) {
                    best = Some((i, at_start, d));
                }
            }
        }
        best.map(|(i, s, _)| (i, s))
    };

    for start in 0..runs.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut chain = runs[start].clone().expect("unused runs exist");
        let mut sources = vec![start];

        // Grow forwards from the chain's end; the joined corner stays on the chain.
        while !closes(&chain, tolerance) {
            let Some((i, at_start)) = nearest(&used, last(&chain)) else {
                break;
            };
            used[i] = true;
            let run = runs[i].as_deref().expect("unused runs exist");
            let run = if at_start {
                run.to_vec()
            } else {
                reversed(run)
            };
            chain.last_mut().expect("runs are not empty").bulge = run[0].bulge;
            chain.extend_from_slice(&run[1..]);
            sources.push(i);
        }
        // Then backwards from its start.
        while !closes(&chain, tolerance) {
            let Some((i, at_start)) = nearest(&used, first(&chain)) else {
                break;
            };
            used[i] = true;
            let run = runs[i].as_deref().expect("unused runs exist");
            let mut run = if at_start {
                reversed(run)
            } else {
                run.to_vec()
            };
            run.pop(); // its end is the chain's start
            run.extend_from_slice(&chain);
            chain = run;
            sources.insert(0, i);
        }

        let closed = closes(&chain, tolerance);
        if closed {
            chain.pop(); // the last point duplicates the first
        } else if sources.len() == 1 {
            continue; // touches nothing: left as it was
        }
        out.push(Joined {
            polyline: Polyline {
                vertices: chain,
                closed,
            },
            sources,
        });
    }
    out
}

/// Whether a chain's two ends meet (and it has enough to it to make a closed shape).
fn closes(chain: &[Vertex], tolerance: f64) -> bool {
    let n = chain.len();
    let enough = n >= 4 || (n == 3 && chain[..2].iter().any(|v| v.bulge != 0.0));
    enough && first(chain).distance_to(last(chain)) <= tolerance
}

// --- Break at intersections --------------------------------------------------------------------

/// What a shape is rebuilt as after it's broken.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Line,
    Arc,
    Circle,
    Polyline,
}

struct Breakable {
    prims: Vec<Prim>,
    closed: bool,
    kind: Kind,
}

fn breakable(shape: &Shape) -> Option<Breakable> {
    let b = |prims, closed, kind| {
        Some(Breakable {
            prims,
            closed,
            kind,
        })
    };
    match shape {
        Shape::Line { start, end } => b(vec![Prim::Line { a: *start, b: *end }], false, Kind::Line),
        Shape::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let sweep = arc_sweep_deg(*start_angle, *end_angle).to_radians();
            let prim = Prim::Arc {
                c: *center,
                r: *radius,
                start: start_angle.to_radians(),
                sweep,
            };
            b(vec![prim], false, Kind::Arc)
        }
        Shape::Circle { center, radius } => {
            let prim = Prim::Arc {
                c: *center,
                r: *radius,
                start: 0.0,
                sweep: TAU,
            };
            b(vec![prim], true, Kind::Circle)
        }
        Shape::Polyline(pl) if pl.vertices.len() >= 2 => {
            b(polyline_prims(pl), pl.closed, Kind::Polyline)
        }
        Shape::Rectangle {
            lower_left,
            width,
            height,
            rotation,
        } => {
            let c = rectangle_corners(*lower_left, *width, *height, *rotation);
            let prims = (0..4)
                .map(|i| Prim::Line {
                    a: c[i],
                    b: c[(i + 1) % 4],
                })
                .collect();
            b(prims, true, Kind::Polyline)
        }
        _ => None,
    }
}

/// Where along an arc (0..=1) the angle `theta` falls, allowing `slack` (a fraction of the arc)
/// past either end. `None` if it's not on the arc.
fn arc_param(start: f64, sweep: f64, theta: f64, slack: f64) -> Option<f64> {
    let d = if sweep >= 0.0 {
        (theta - start).rem_euclid(TAU)
    } else {
        (start - theta).rem_euclid(TAU)
    };
    let t = d / sweep.abs();
    if t <= 1.0 + slack {
        Some(t.min(1.0))
    } else if (TAU - d) / sweep.abs() <= slack {
        Some(0.0) // just before the start
    } else {
        None
    }
}

/// Tiny allowance (mm) so that crossings exactly at an end are found despite rounding.
const REACH: f64 = 1e-7;

/// Crossing points of two pieces, as (parameter on p, parameter on q, point).
fn intersect(p: &Prim, q: &Prim) -> Vec<(f64, f64, Point)> {
    match (p, q) {
        (Prim::Line { .. }, Prim::Arc { .. }) => intersect(q, p)
            .into_iter()
            .map(|(a, b, pt)| (b, a, pt))
            .collect(),
        (Prim::Line { a: a1, b: b1 }, Prim::Line { a: a2, b: b2 }) => {
            let (d1, d2) = (sub(*b1, *a1), sub(*b2, *a2));
            let (l1, l2) = (d1.x.hypot(d1.y), d2.x.hypot(d2.y));
            let den = cross(d1, d2);
            if l1 == 0.0 || l2 == 0.0 || den.abs() <= 1e-12 * l1 * l2 {
                return Vec::new(); // parallel (or overlapping): no single crossing point
            }
            let w = sub(*a2, *a1);
            let t = cross(w, d2) / den;
            let u = cross(w, d1) / den;
            let (e1, e2) = (REACH / l1, REACH / l2);
            if (-e1..=1.0 + e1).contains(&t) && (-e2..=1.0 + e2).contains(&u) {
                let t = t.clamp(0.0, 1.0);
                vec![(t, u.clamp(0.0, 1.0), p.at(t))]
            } else {
                Vec::new()
            }
        }
        (Prim::Arc { c, r, start, sweep }, Prim::Line { a, b }) => {
            let d = sub(*b, *a);
            let len = d.x.hypot(d.y);
            if len == 0.0 {
                return Vec::new();
            }
            // Closest point on the line to the centre, then either side of it.
            let f = sub(*a, *c);
            let mid = -dot(f, d) / (len * len);
            let foot = Point::new(a.x + d.x * mid, a.y + d.y * mid);
            let dist = foot.distance_to(*c);
            if dist > r + REACH {
                return Vec::new();
            }
            let half = (r * r - dist * dist).max(0.0).sqrt() / len;
            let ts: &[f64] = if half * len <= REACH {
                &[mid]
            } else {
                &[mid - half, mid + half]
            };
            let e = REACH / len;
            let slack = REACH / (r * sweep.abs());
            ts.iter()
                .filter(|t| (-e..=1.0 + e).contains(*t))
                .filter_map(|&u| {
                    let u = u.clamp(0.0, 1.0);
                    let pt = q.at(u);
                    let theta = (pt.y - c.y).atan2(pt.x - c.x);
                    arc_param(*start, *sweep, theta, slack).map(|t| (t, u, pt))
                })
                .collect()
        }
        (
            Prim::Arc {
                c: c1,
                r: r1,
                start: s1,
                sweep: w1,
            },
            Prim::Arc {
                c: c2,
                r: r2,
                start: s2,
                sweep: w2,
            },
        ) => {
            let d = c1.distance_to(*c2);
            if d <= 1e-12 || d > r1 + r2 + REACH || d < (r1 - r2).abs() - REACH {
                return Vec::new(); // same centre, apart, or one inside the other
            }
            let along = (d * d + r1 * r1 - r2 * r2) / (2.0 * d);
            let h = (r1 * r1 - along * along).max(0.0).sqrt();
            let (ux, uy) = ((c2.x - c1.x) / d, (c2.y - c1.y) / d);
            let base = Point::new(c1.x + ux * along, c1.y + uy * along);
            let mut pts = vec![Point::new(base.x - uy * h, base.y + ux * h)];
            if h > REACH {
                pts.push(Point::new(base.x + uy * h, base.y - ux * h));
            }
            let (sl1, sl2) = (REACH / (r1 * w1.abs()), REACH / (r2 * w2.abs()));
            pts.into_iter()
                .filter_map(|pt| {
                    let t = arc_param(*s1, *w1, (pt.y - c1.y).atan2(pt.x - c1.x), sl1)?;
                    let u = arc_param(*s2, *w2, (pt.y - c2.y).atan2(pt.x - c2.x), sl2)?;
                    Some((t, u, pt))
                })
                .collect()
        }
    }
}

/// A cut: which piece, how far along it, and the crossing point.
#[derive(Debug, Clone, Copy)]
struct Cut {
    seg: usize,
    t: f64,
    point: Point,
}

/// The result of breaking: for each input shape that was cut (index into the input), its pieces.
#[derive(Debug, Clone, PartialEq)]
pub struct Broken {
    pub source: usize,
    pub pieces: Vec<Shape>,
}

/// Breaks shapes at every point where two of them cross or touch. Lines, arcs, circles,
/// polylines and rectangles can be broken; a shape is not cut where it crosses itself. Returns
/// the broken shapes and the number of distinct crossing points.
pub fn break_at_intersections(shapes: &[Shape]) -> (Vec<Broken>, usize) {
    let parts: Vec<Option<Breakable>> = shapes.iter().map(breakable).collect();
    let mut cuts: Vec<Vec<Cut>> = vec![Vec::new(); shapes.len()];
    let mut crossings: Vec<Point> = Vec::new();

    for i in 0..parts.len() {
        for j in i + 1..parts.len() {
            let (Some(a), Some(b)) = (&parts[i], &parts[j]) else {
                continue;
            };
            for (si, p) in a.prims.iter().enumerate() {
                for (sj, q) in b.prims.iter().enumerate() {
                    for (t, u, point) in intersect(p, q) {
                        cuts[i].push(Cut { seg: si, t, point });
                        cuts[j].push(Cut {
                            seg: sj,
                            t: u,
                            point,
                        });
                        if !crossings
                            .iter()
                            .any(|c| c.distance_to(point) <= EDIT_TOLERANCE)
                        {
                            crossings.push(point);
                        }
                    }
                }
            }
        }
    }

    let broken = parts
        .iter()
        .zip(cuts)
        .enumerate()
        .filter_map(|(source, (part, cuts))| {
            let pieces = split(part.as_ref()?, cuts)?;
            Some(Broken { source, pieces })
        })
        .collect();
    (broken, crossings.len())
}

/// Splits one shape at its cuts. `None` if it doesn't change.
fn split(shape: &Breakable, mut cuts: Vec<Cut>) -> Option<Vec<Shape>> {
    let prims = &shape.prims;
    // Distance along the shape to the start of each piece, and in total.
    let mut starts = Vec::with_capacity(prims.len());
    let mut total = 0.0;
    for p in prims {
        starts.push(total);
        total += p.length();
    }
    let along = |c: &Cut| starts[c.seg] + prims[c.seg].length() * c.t;

    cuts.sort_by(|a, b| along(a).total_cmp(&along(b)));
    let mut kept: Vec<Cut> = Vec::new();
    for c in cuts {
        let s = along(&c);
        let at_end = !shape.closed && (s <= EDIT_TOLERANCE || total - s <= EDIT_TOLERANCE);
        let repeat = kept.last().is_some_and(|k| s - along(k) <= EDIT_TOLERANCE);
        if !at_end && !repeat {
            kept.push(c);
        }
    }
    if shape.closed
        && kept.len() > 1
        && total - along(&kept[kept.len() - 1]) + along(&kept[0]) <= EDIT_TOLERANCE
    {
        kept.pop(); // the same point, either side of where the shape starts
    }

    let n = prims.len();
    let runs: Vec<(Cut, Cut, bool)> = if shape.closed {
        match kept.len() {
            0 => return None,
            1 if shape.kind == Kind::Circle => return None,
            1 => vec![(kept[0], kept[0], true)],
            k => (0..k)
                .map(|i| (kept[i], kept[(i + 1) % k], i + 1 == k))
                .collect(),
        }
    } else {
        if kept.is_empty() {
            return None;
        }
        let start = Cut {
            seg: 0,
            t: 0.0,
            point: prims[0].at(0.0),
        };
        let end = Cut {
            seg: n - 1,
            t: 1.0,
            point: prims[n - 1].at(1.0),
        };
        let mut points = vec![start];
        points.extend(kept);
        points.push(end);
        points.windows(2).map(|w| (w[0], w[1], false)).collect()
    };

    let pieces = runs
        .into_iter()
        .filter_map(|(from, to, wraps)| {
            let piece = walk(prims, from, to, wraps);
            build(shape.kind, &piece, from.point, to.point)
        })
        .collect();
    Some(pieces)
}

/// The pieces from cut `from` to cut `to`, going round past the shape's start if `wraps`
/// (a closed shape) or the cuts are out of order.
fn walk(prims: &[Prim], from: Cut, to: Cut, wraps: bool) -> Vec<Prim> {
    let n = prims.len();
    let mut out = Vec::new();
    if !wraps && from.seg == to.seg && from.t <= to.t {
        out.push(prims[from.seg].part(from.t, to.t));
    } else {
        out.push(prims[from.seg].part(from.t, 1.0));
        let mut s = (from.seg + 1) % n;
        while s != to.seg {
            out.push(prims[s]);
            s = (s + 1) % n;
        }
        out.push(prims[to.seg].part(0.0, to.t));
    }
    out.retain(|p| p.length() > 1e-9);
    out
}

/// A piece as a shape of the original's kind, with its ends exactly on the cut points.
fn build(kind: Kind, piece: &[Prim], from: Point, to: Point) -> Option<Shape> {
    if piece.is_empty() {
        return None;
    }
    match kind {
        Kind::Line => Some(Shape::Line {
            start: from,
            end: to,
        }),
        Kind::Arc | Kind::Circle => {
            // A circle's piece that runs past 0° comes in two parts on the same circle.
            let Prim::Arc { c, r, start, .. } = piece[0] else {
                return None;
            };
            let sweep = piece
                .iter()
                .map(|p| match p {
                    Prim::Arc { sweep, .. } => *sweep,
                    Prim::Line { .. } => 0.0,
                })
                .sum();
            Some(Prim::Arc { c, r, start, sweep }.to_shape())
        }
        Kind::Polyline => {
            let mut vertices: Vec<Vertex> = piece
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let at = if i == 0 { from } else { p.at(0.0) };
                    Vertex {
                        x: at.x,
                        y: at.y,
                        bulge: p.bulge(),
                    }
                })
                .collect();
            vertices.push(Vertex::line(to.x, to.y));
            Some(Shape::Polyline(Polyline {
                vertices,
                closed: false,
            }))
        }
    }
}
