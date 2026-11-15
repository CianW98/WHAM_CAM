//! Turns the document into simple point lists the interface can draw.
//! Arcs and circles are approximated with short straight segments for the screen only;
//! toolpaths and G-code keep true arcs.

use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::geometry::{Point, Polyline, Shape};

/// Maximum distance between a true arc and its on-screen approximation, in mm.
pub const DISPLAY_TOLERANCE: f64 = 0.01;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayData {
    pub shapes: Vec<DisplayShape>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayShape {
    /// The drawing object this came from.
    pub id: u32,
    /// Points in order, as [x, y] pairs, in mm.
    pub points: Vec<[f64; 2]>,
    /// True if the last point joins back to the first.
    pub closed: bool,
}

pub fn display_data(doc: &Document) -> DisplayData {
    DisplayData {
        shapes: doc
            .objects()
            .map(|obj| {
                let (points, closed) = shape_points(&obj.shape);
                DisplayShape {
                    id: obj.id,
                    points,
                    closed,
                }
            })
            .collect(),
    }
}

fn shape_points(shape: &Shape) -> (Vec<[f64; 2]>, bool) {
    match shape {
        Shape::Circle { center, radius } => {
            let mut pts = Vec::new();
            push_arc(&mut pts, *center, *radius, 0.0, TAU);
            pts.pop(); // last point duplicates the first; the shape is closed
            (pts, true)
        }
        Shape::Polyline(pl) => (polyline_points(pl), pl.closed),
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
fn bulge_arc(a: Point, b: Point, bulge: f64) -> Option<(Point, f64, f64, f64)> {
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

/// Appends points along an arc, including both the start and end points.
fn push_arc(pts: &mut Vec<[f64; 2]>, center: Point, radius: f64, start: f64, sweep: f64) {
    let max_step = if radius > DISPLAY_TOLERANCE {
        2.0 * (1.0 - DISPLAY_TOLERANCE / radius).acos()
    } else {
        TAU / 8.0
    };
    let steps = ((sweep.abs() / max_step).ceil() as usize).max(2);
    for i in 0..=steps {
        let angle = start + sweep * (i as f64) / (steps as f64);
        pts.push([
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        ]);
    }
}
