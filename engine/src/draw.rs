//! Drawing by clicking: building shapes from clicked points, snapping, and checking that a
//! shape is valid before it goes into the document.
//!
//! The interface collects the clicked points (a `Draft`) and asks the engine what shape they
//! make. All geometry stays here.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::geometry::{
    Point, Polyline, Shape, Vertex, angle_deg, arc_sweep_deg, bulge_arc, cross, dot, mid,
    on_circle, rectangle_corners, sub,
};

/// Points closer than this (mm) count as the same point.
const SAME_POINT: f64 = 1e-9;

/// The drawing tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawTool {
    /// Start, end.
    Line,
    /// Start, end, then centre (slides along the line halfway between the ends).
    /// Always counter-clockwise from start to end.
    Arc,
    /// Centre, then a point on the circle.
    Circle,
    /// Two opposite corners.
    Rectangle,
    /// Any number of points; segments can be straight or tangent arcs.
    Polyline,
    /// Any number of points, making one point list.
    Points,
    /// A dimension, its kind worked out from what is clicked (see [`crate::dimension`]): a
    /// circle → diameter, an arc → radius, two points → aligned, a line then another line →
    /// angle (or the distance between them if parallel), a line then elsewhere → its length.
    /// Then a last click places it.
    Dimension,
}

impl DrawTool {
    /// How many clicks finish the shape, or `None` if the user finishes it (Enter).
    pub fn clicks_needed(self) -> Option<usize> {
        match self {
            DrawTool::Line | DrawTool::Circle | DrawTool::Rectangle => Some(2),
            DrawTool::Arc => Some(3),
            // The engine says when each click finishes (see `Project::dimension_click`).
            DrawTool::Polyline | DrawTool::Points | DrawTool::Dimension => None,
        }
    }
}

/// A shape being drawn: the points clicked so far.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Draft {
    pub tool: DrawTool,
    pub points: Vec<Point>,
    /// Polyline only: `arcs[i]` is true if the segment from `points[i]` to `points[i + 1]` is a
    /// tangent arc. Missing entries mean straight.
    #[serde(default)]
    pub arcs: Vec<bool>,
    /// Polyline only: whether the next segment (to the cursor, or the closing segment) is a
    /// tangent arc.
    #[serde(default)]
    pub arc_next: bool,
    /// Dimension only: for each click in `points`, what it landed on (a shape's point, a line,
    /// or a circle/arc), or `None` for the free last click.
    #[serde(default)]
    pub targets: Vec<Option<crate::dimension::Target>>,
}

/// What a point snapped to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SnapKind {
    /// Not snapped: the point is exactly where the cursor is.
    None,
    Grid,
    /// End point of a line or arc, a polyline corner, or a point.
    End,
    Mid,
    Centre,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Snap {
    pub point: Point,
    pub kind: SnapKind,
}

/// Which snaps are switched on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapOptions {
    pub grid: bool,
    pub objects: bool,
}

// ---------------------------------------------------------------------------------------------
// Building shapes

/// Arc from `start` to `end`, counter-clockwise, with its centre on the line halfway between
/// them, as close as possible to `near`.
pub fn arc_start_end_centre(start: Point, end: Point, near: Point) -> Option<Shape> {
    let chord = sub(end, start);
    let len = chord.x.hypot(chord.y);
    if len <= SAME_POINT {
        return None;
    }
    // Unit vector across the chord, and the centre projected onto the bisector.
    let n = Point::new(-chord.y / len, chord.x / len);
    let m = mid(start, end);
    let along = (near.x - m.x) * n.x + (near.y - m.y) * n.y;
    let center = Point::new(m.x + n.x * along, m.y + n.y * along);
    let radius = center.distance_to(start);
    let start_angle = angle_deg(center, start);
    let mut end_angle = angle_deg(center, end);
    if end_angle <= start_angle {
        end_angle += 360.0;
    }
    Some(Shape::Arc {
        center,
        radius,
        start_angle,
        end_angle,
    })
}

/// Rectangle (a PolyRectangle) from two opposite corners, unrotated.
pub fn rectangle_from_corners(a: Point, b: Point) -> Shape {
    Shape::Rectangle {
        lower_left: Point::new(a.x.min(b.x), a.y.min(b.y)),
        width: (a.x - b.x).abs(),
        height: (a.y - b.y).abs(),
        rotation: 0.0,
    }
}

/// Rotates `v` by `angle` radians counter-clockwise.
fn rotate(v: Point, angle: f64) -> Point {
    let (s, c) = angle.sin_cos();
    Point::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

/// Direction of travel at the end of the segment from `a` to `b` with `bulge`.
fn end_direction(a: Point, b: Point, bulge: f64) -> Point {
    // The end tangent is the chord turned by half the arc's sweep (sweep = 4·atan(bulge)).
    rotate(sub(b, a), 2.0 * bulge.atan())
}

/// Bulge of an arc from `from` to `to` that starts in direction `dir` (a tangent arc).
pub fn tangent_bulge(dir: Point, from: Point, to: Point) -> f64 {
    let c = sub(to, from);
    // Signed angle from the start direction to the chord is half the sweep.
    let alpha = cross(dir, c).atan2(dot(dir, c));
    (alpha / 2.0).tan()
}

/// Builds a polyline from points and per-segment arc flags. Repeated points are dropped. A
/// segment marked as an arc becomes a tangent arc following the previous segment; the first
/// segment has nothing to follow, so it is always straight.
fn polyline_from(points: &[Point], arcs: &[bool], closed: bool) -> Polyline {
    let mut pts: Vec<Point> = Vec::new();
    let mut flags: Vec<bool> = Vec::new();
    for (i, p) in points.iter().enumerate() {
        let arc = arcs.get(i).copied().unwrap_or(false);
        match pts.last() {
            Some(last) if last.distance_to(*p) <= SAME_POINT => {
                // Same point again: keep the later segment kind.
                *flags.last_mut().unwrap() = arc;
            }
            _ => {
                pts.push(*p);
                flags.push(arc);
            }
        }
    }
    let n = pts.len();
    let mut vertices: Vec<Vertex> = pts.iter().map(|p| Vertex::line(p.x, p.y)).collect();
    let segments = if closed { n } else { n.saturating_sub(1) };
    let mut dir: Option<Point> = None;
    for i in 0..segments {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        if let (true, Some(d)) = (flags[i], dir) {
            vertices[i].bulge = tangent_bulge(d, a, b);
        }
        dir = Some(end_direction(a, b, vertices[i].bulge));
    }
    Polyline { vertices, closed }
}

/// The shape a draft makes, with `cursor` (if any) as the next point. Lenient: it shows
/// whatever can be shown so far (e.g. a straight line while an arc has only two points), for
/// the rubber-band preview. Returns `None` if there's nothing to show.
pub fn preview(draft: &Draft, cursor: Option<Point>) -> Option<Shape> {
    let mut pts = draft.points.clone();
    pts.extend(cursor);
    if pts.is_empty() {
        return None;
    }
    let two = || (pts.len() >= 2).then(|| (pts[0], pts[1]));
    match draft.tool {
        DrawTool::Line => two().map(|(start, end)| Shape::Line { start, end }),
        DrawTool::Circle => two().map(|(center, p)| Shape::Circle {
            center,
            radius: center.distance_to(p),
        }),
        DrawTool::Rectangle => two().map(|(a, b)| rectangle_from_corners(a, b)),
        DrawTool::Arc => {
            if pts.len() >= 3 {
                arc_start_end_centre(pts[0], pts[1], pts[2])
            } else {
                two().map(|(start, end)| Shape::Line { start, end })
            }
        }
        DrawTool::Polyline => {
            let mut arcs = draft.arcs.clone();
            arcs.resize(draft.points.len().saturating_sub(1), false);
            arcs.push(draft.arc_next);
            (pts.len() >= 2).then(|| Shape::Polyline(polyline_from(&pts, &arcs, false)))
        }
        DrawTool::Points => Some(Shape::Points { points: pts }),
        // Dimensions aren't shapes: see `Project::preview`.
        DrawTool::Dimension => None,
    }
}

/// The finished shape from a draft, checked with [`validate`]. A polyline whose last point is
/// back on its first point becomes a closed polyline.
pub fn finish(draft: &Draft) -> Result<Shape, String> {
    let pts = &draft.points;
    if let Some(n) = draft.tool.clicks_needed().filter(|&n| pts.len() != n) {
        return Err(format!("This shape needs {n} points."));
    }
    let shape = match draft.tool {
        DrawTool::Arc => arc_start_end_centre(pts[0], pts[1], pts[2])
            .ok_or("The start and end of an arc can't be the same point.")?,
        DrawTool::Polyline => {
            let mut arcs = draft.arcs.clone();
            arcs.resize(pts.len().saturating_sub(1), false);
            let mut pts = pts.clone();
            let mut closed = false;
            if pts.len() >= 3 && pts[0].distance_to(*pts.last().unwrap()) <= SAME_POINT {
                // Drop the repeated first point. The segment that ran back to it keeps its
                // place in `arcs` and becomes the closing segment.
                pts.pop();
                closed = true;
            }
            Shape::Polyline(polyline_from(&pts, &arcs, closed))
        }
        _ => preview(draft, None).ok_or("Nothing to draw.")?,
    };
    validate(&shape)?;
    Ok(shape)
}

// ---------------------------------------------------------------------------------------------
// Checking shapes

fn finite(p: Point) -> bool {
    p.x.is_finite() && p.y.is_finite()
}

/// Checks a shape can go into the drawing. Errors are plain-English messages.
pub fn validate(shape: &Shape) -> Result<(), String> {
    const NOT_NUMBER: &str = "Every value must be a number.";
    let ok = |cond: bool, msg: &str| if cond { Ok(()) } else { Err(msg.to_string()) };
    match shape {
        Shape::Line { start, end } => {
            ok(finite(*start) && finite(*end), NOT_NUMBER)?;
            ok(
                start.distance_to(*end) > SAME_POINT,
                "A line's start and end can't be the same point.",
            )
        }
        Shape::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            ok(
                finite(*center)
                    && radius.is_finite()
                    && start_angle.is_finite()
                    && end_angle.is_finite(),
                NOT_NUMBER,
            )?;
            ok(*radius > 0.0, "The radius must be more than 0.")?;
            ok(
                (end_angle - start_angle).abs() > 1e-9,
                "An arc's start and end angles can't be the same.",
            )
        }
        Shape::Circle { center, radius } => {
            ok(finite(*center) && radius.is_finite(), NOT_NUMBER)?;
            ok(*radius > 0.0, "The radius must be more than 0.")
        }
        Shape::Rectangle {
            lower_left,
            width,
            height,
            rotation,
        } => {
            ok(
                finite(*lower_left)
                    && width.is_finite()
                    && height.is_finite()
                    && rotation.is_finite(),
                NOT_NUMBER,
            )?;
            ok(
                *width > 0.0 && *height > 0.0,
                "A rectangle's width and height must be more than 0.",
            )
        }
        Shape::Polyline(pl) => {
            ok(
                pl.vertices
                    .iter()
                    .all(|v| finite(v.point()) && v.bulge.is_finite()),
                NOT_NUMBER,
            )?;
            ok(
                pl.vertices.len() >= 2,
                "A polyline needs at least 2 different points.",
            )
        }
        Shape::Points { points } => {
            ok(points.iter().all(|p| finite(*p)), NOT_NUMBER)?;
            ok(!points.is_empty(), "A point list needs at least 1 point.")
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Snapping

/// Points a shape offers for object snapping.
/// The points a shape offers for object snapping, in a fixed order (dimensions refer to them by
/// kind and position in this list).
pub fn snap_features(shape: &Shape) -> Vec<(Point, SnapKind)> {
    let mut out = Vec::new();
    snap_points(shape, &mut out);
    out
}

fn snap_points(shape: &Shape, out: &mut Vec<(Point, SnapKind)>) {
    use SnapKind::*;
    let polar = |c: Point, r: f64, deg: f64| on_circle(c, r, deg.to_radians());
    match shape {
        Shape::Line { start, end } => {
            out.extend([(*start, End), (*end, End), (mid(*start, *end), Mid)]);
        }
        Shape::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let sweep = arc_sweep_deg(*start_angle, *end_angle);
            out.extend([
                (polar(*center, *radius, *start_angle), End),
                (polar(*center, *radius, *end_angle), End),
                (polar(*center, *radius, start_angle + sweep / 2.0), Mid),
                (*center, Centre),
            ]);
        }
        Shape::Circle { center, .. } => out.push((*center, Centre)),
        Shape::Rectangle {
            lower_left,
            width,
            height,
            rotation,
        } => {
            let c = rectangle_corners(*lower_left, *width, *height, *rotation);
            out.extend(c.iter().map(|p| (*p, End)));
            out.extend((0..4).map(|i| (mid(c[i], c[(i + 1) % 4]), Mid)));
            out.push((mid(c[0], c[2]), Centre));
        }
        Shape::Polyline(pl) => {
            let n = pl.vertices.len();
            out.extend(pl.vertices.iter().map(|v| (v.point(), End)));
            let segments = if pl.closed { n } else { n.saturating_sub(1) };
            for i in 0..segments {
                let (a, b) = (pl.vertices[i], pl.vertices[(i + 1) % n]);
                match bulge_arc(a.point(), b.point(), a.bulge) {
                    Some(arc) => out.extend([(arc.mid_point(), Mid), (arc.center, Centre)]),
                    Option::None => out.push((mid(a.point(), b.point()), Mid)),
                }
            }
        }
        Shape::Points { points } => out.extend(points.iter().map(|p| (*p, End))),
    }
}

/// Snaps a cursor position. Object snaps (end, mid, centre) within `tolerance` mm win;
/// otherwise the nearest grid point; otherwise the point itself. `extra` are points of the
/// shape being drawn, which can be snapped to as end points (e.g. to close a polyline).
pub fn snap(
    doc: &Document,
    hidden: &BTreeSet<String>,
    at: Point,
    tolerance: f64,
    options: SnapOptions,
    extra: &[Point],
) -> Snap {
    if options.objects
        && let Some(s) = object_snap(doc, hidden, at, tolerance, extra, |_| true)
    {
        return s;
    }
    grid_snap(doc, at, options)
}

/// The nearest object snap point within `tolerance` of `at` whose kind `allow`s, if any, on
/// layers not named in `hidden`. `extra` points count as end points.
pub fn object_snap(
    doc: &Document,
    hidden: &BTreeSet<String>,
    at: Point,
    tolerance: f64,
    extra: &[Point],
    allow: impl Fn(SnapKind) -> bool,
) -> Option<Snap> {
    let mut candidates = Vec::new();
    for layer in doc.layers.iter().filter(|l| !hidden.contains(&l.name)) {
        for obj in &layer.objects {
            snap_points(&obj.shape, &mut candidates);
        }
    }
    candidates.extend(extra.iter().map(|p| (*p, SnapKind::End)));
    candidates
        .into_iter()
        .filter(|(_, kind)| allow(*kind))
        .map(|(p, kind)| (p.distance_to(at), p, kind))
        .filter(|(d, _, _)| *d <= tolerance)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, point, kind)| Snap { point, kind })
}

/// The nearest grid point (if the grid snap is on), or `at` itself.
pub fn grid_snap(doc: &Document, at: Point, options: SnapOptions) -> Snap {
    let g = doc.grid_spacing;
    if options.grid && g > 0.0 {
        return Snap {
            // `+ 0.0` turns -0.0 into 0.0.
            point: Point::new((at.x / g).round() * g + 0.0, (at.y / g).round() * g + 0.0),
            kind: SnapKind::Grid,
        };
    }
    Snap {
        point: at,
        kind: SnapKind::None,
    }
}
