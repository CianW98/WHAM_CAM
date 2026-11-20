//! Shared pieces for building toolpaths along shapes (Pocket and Profile): turning shapes into
//! closed or open paths, walking round loops, and the move builder (retract, approach, cut).

use std::f64::consts::TAU;

use crate::geometry::{
    Point, Polyline, Shape, Vertex, arc_sweep_deg, bulge_arc, bulge_from_sweep, dot, on_circle,
    rectangle_corners, sub,
};
use crate::operation::CommonSettings;
use crate::toolpath::{Move, Point3};

/// A selected shape as a closed outline, or why it can't be pocketed.
pub fn closed_outline(id: u32, shape: &Shape) -> Result<Polyline, String> {
    match shape {
        Shape::Polyline(pl) if pl.closed && pl.vertices.len() >= 2 => Ok(pl.clone()),
        Shape::Polyline(_) => Err(format!(
            "Shape {id} is an open polyline. Pockets need closed shapes (use Close polyline or Join)."
        )),
        Shape::Rectangle {
            lower_left,
            width,
            height,
            rotation,
        } => Ok(Polyline {
            vertices: rectangle_corners(*lower_left, *width, *height, *rotation)
                .iter()
                .map(|c| Vertex::line(c.x, c.y))
                .collect(),
            closed: true,
        }),
        Shape::Circle { center, radius } => Ok(Polyline {
            vertices: vec![
                Vertex {
                    x: center.x + radius,
                    y: center.y,
                    bulge: 1.0,
                },
                Vertex {
                    x: center.x - radius,
                    y: center.y,
                    bulge: 1.0,
                },
            ],
            closed: true,
        }),
        _ => Err(format!(
            "Shape {id} is a {}: pockets need closed shapes.",
            shape.type_name()
        )),
    }
}

// --- Walking loops ------------------------------------------------------------------------------

/// The point on a polyline segment nearest to `p`, and how far along the segment it is (0..1).
pub(crate) fn nearest_on_segment(a: Vertex, b: Vertex, p: Point) -> (Point, f64) {
    match bulge_arc(a.point(), b.point(), a.bulge) {
        None => {
            let d = sub(b.point(), a.point());
            let len2 = dot(d, d);
            let t = if len2 == 0.0 {
                0.0
            } else {
                (dot(sub(p, a.point()), d) / len2).clamp(0.0, 1.0)
            };
            (Point::new(a.x + d.x * t, a.y + d.y * t), t)
        }
        Some(arc) => {
            let theta = (p.y - arc.center.y).atan2(p.x - arc.center.x);
            let off = if arc.sweep >= 0.0 {
                (theta - arc.start).rem_euclid(TAU)
            } else {
                (arc.start - theta).rem_euclid(TAU)
            };
            let t = off / arc.sweep.abs();
            let t = if t <= 1.0 {
                t
            } else if p.distance_to(a.point()) <= p.distance_to(b.point()) {
                0.0
            } else {
                1.0
            };
            (
                on_circle(arc.center, arc.radius, arc.start + arc.sweep * t),
                t,
            )
        }
    }
}

/// The nearest point on a closed loop to `p`: (segment index, parameter, point, distance).
/// Ties go to the earlier segment, so results are repeatable.
pub(crate) fn nearest_on_loop(pl: &Polyline, p: Point) -> (usize, f64, Point, f64) {
    let n = pl.vertices.len();
    let mut best = (0, 0.0, pl.vertices[0].point(), f64::INFINITY);
    for i in 0..n {
        let (q, t) = nearest_on_segment(pl.vertices[i], pl.vertices[(i + 1) % n], p);
        let d = q.distance_to(p);
        if d < best.3 - 1e-9 {
            best = (i, t, q, d);
        }
    }
    best
}

/// The bulge of the part of a segment between parameters t0 and t1.
pub(crate) fn part_bulge(a: Vertex, b: Vertex, t0: f64, t1: f64) -> f64 {
    match bulge_arc(a.point(), b.point(), a.bulge) {
        Some(arc) => bulge_from_sweep(arc.sweep * (t1 - t0)),
        None => 0.0,
    }
}

/// The closed loop re-started at its point nearest `p` (splitting a segment if needed).
pub(crate) fn start_nearest(pl: &Polyline, p: Point) -> Polyline {
    let n = pl.vertices.len();
    let (i, t, q, _) = nearest_on_loop(pl, p);
    let v = &pl.vertices;
    let (a, b) = (v[i], v[(i + 1) % n]);
    let mut out = Vec::with_capacity(n + 1);
    if t <= 1e-9 {
        // Starts exactly at vertex i.
        out.extend((0..n).map(|k| v[(i + k) % n]));
    } else if t >= 1.0 - 1e-9 {
        out.extend((0..n).map(|k| v[(i + 1 + k) % n]));
    } else {
        out.push(Vertex {
            x: q.x,
            y: q.y,
            bulge: part_bulge(a, b, t, 1.0),
        });
        out.extend((1..n).map(|k| v[(i + k) % n]));
        out.push(Vertex {
            x: a.x,
            y: a.y,
            bulge: part_bulge(a, b, 0.0, t),
        });
    }
    Polyline {
        vertices: out,
        closed: true,
    }
}

// --- Building the moves -------------------------------------------------------------------------

pub(crate) struct Builder<'a> {
    pub c: &'a CommonSettings,
    pub moves: Vec<Move>,
    /// Where the tool is (None before the first move).
    pub at: Option<Point3>,
}

impl Builder<'_> {
    pub(crate) fn push(&mut self, m: Move) {
        self.at = Some(m.to());
        self.moves.push(m);
    }

    pub(crate) fn xy(&self) -> Point {
        self.at
            .map_or(Point::new(0.0, 0.0), |a| Point::new(a.x, a.y))
    }

    pub(crate) fn retract(&mut self) {
        let clear = self.c.clearance_plane;
        if let Some(a) = self.at
            && a.z < clear
        {
            self.push(Move::Rapid {
                to: Point3::new(a.x, a.y, clear),
            });
        }
    }

    /// Up to clearance, across to `p`, rapid down to fast plunge height above `above_z` (the
    /// level already cut there), then feed down to `z`.
    pub(crate) fn approach(&mut self, p: Point, above_z: f64, z: f64) {
        let clear = self.c.clearance_plane;
        self.retract();
        let here = self.at;
        if here.is_none_or(|a| (a.x - p.x).hypot(a.y - p.y) > 1e-9) {
            self.push(Move::Rapid {
                to: Point3::new(p.x, p.y, clear),
            });
        }
        let fast = above_z + self.c.fast_plunge_height;
        if fast < clear {
            self.push(Move::Rapid {
                to: Point3::new(p.x, p.y, fast),
            });
        }
        self.push(Move::Feed {
            to: Point3::new(p.x, p.y, z),
            feed: self.c.plunge_feedrate,
        });
    }

    /// Cuts round a closed loop at height `z`, starting and ending at its first vertex.
    pub(crate) fn cut_loop(&mut self, pl: &Polyline, z: f64) {
        let v = &pl.vertices;
        let n = v.len();
        for i in 0..n {
            let (a, b) = (v[i], v[(i + 1) % n]);
            self.cut_segment(a.point(), b.point(), a.bulge, z);
        }
    }

    /// Cuts along an open path at height `z`, from its first vertex to its last.
    pub(crate) fn cut_open(&mut self, pl: &Polyline, z: f64) {
        for w in pl.vertices.windows(2) {
            self.cut_segment(w[0].point(), w[1].point(), w[0].bulge, z);
        }
    }

    /// One straight or arc move from `a` to `b` at height `z` (nothing if it has no length).
    fn cut_segment(&mut self, a: Point, b: Point, bulge: f64, z: f64) {
        let to = Point3::new(b.x, b.y, z);
        let feed = self.c.cut_feedrate;
        match bulge_arc(a, b, bulge) {
            Some(arc) => self.push(Move::Arc {
                to,
                center_x: arc.center.x,
                center_y: arc.center.y,
                ccw: arc.sweep > 0.0,
                feed,
            }),
            None => {
                if a.distance_to(b) > 1e-9 {
                    self.push(Move::Feed { to, feed });
                }
            }
        }
    }
}

/// A selected shape as a path to profile: closed shapes as closed outlines, lines, arcs and open
/// polylines as open paths. Errors say why a shape can't be profiled.
pub fn profile_path(id: u32, shape: &Shape) -> Result<Polyline, String> {
    match shape {
        Shape::Line { start, end } => Ok(Polyline {
            vertices: vec![Vertex::line(start.x, start.y), Vertex::line(end.x, end.y)],
            closed: false,
        }),
        Shape::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let sweep = arc_sweep_deg(*start_angle, *end_angle).to_radians();
            let a0 = start_angle.to_radians();
            // Arcs over half a circle go in two halves, so no bulge is huge (or infinite).
            let parts = if sweep > std::f64::consts::PI { 2 } else { 1 };
            let part = sweep / parts as f64;
            let mut vertices: Vec<Vertex> = (0..parts)
                .map(|k| {
                    let q = on_circle(*center, *radius, a0 + part * k as f64);
                    Vertex {
                        x: q.x,
                        y: q.y,
                        bulge: bulge_from_sweep(part),
                    }
                })
                .collect();
            let b = on_circle(*center, *radius, a0 + sweep);
            vertices.push(Vertex::line(b.x, b.y));
            Ok(Polyline {
                vertices,
                closed: false,
            })
        }
        Shape::Polyline(pl) if !pl.closed && pl.vertices.len() >= 2 => Ok(pl.clone()),
        Shape::Polyline(pl) if !pl.closed => {
            Err(format!("Shape {id} is a polyline with no length."))
        }
        Shape::Points { .. } => Err(format!("Shape {id} is a point list: it can't be profiled.")),
        _ => closed_outline(id, shape),
    }
}
