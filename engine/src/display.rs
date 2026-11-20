//! Turns the document into simple point lists the interface can draw.
//! Arcs and circles are approximated with short straight segments for the screen only; the
//! drawing itself and the toolpaths keep the exact arcs.

use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

use crate::document::{Document, LAYER_PALETTE};
use crate::geometry::{
    BulgeArc, Point, Polyline, Shape, angle_deg, arc_sweep_deg, bulge_arc, rectangle_corners,
};
use crate::toolpath::{Move, Point3, Toolpath};

/// Maximum distance between a true arc and its on-screen approximation, in mm.
pub const DISPLAY_TOLERANCE: f64 = 0.01;
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
            // Always anticlockwise from start to end, including across 0° (e.g. 300° to 30°).
            let sweep = arc_sweep_deg(*start_angle, *end_angle);
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
        if let Some(BulgeArc {
            center,
            radius,
            start,
            sweep,
        }) = bulge_arc(a.point(), b.point(), a.bulge)
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

/// Appends points along an arc (angles in radians), including both the start and end points.
fn push_arc(pts: &mut Vec<[f64; 2]>, center: Point, radius: f64, start: f64, sweep: f64) {
    let steps = arc_steps(radius, sweep);
    for i in 0..=steps {
        let angle = start + sweep * (i as f64) / (steps as f64);
        pts.push([
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        ]);
    }
}

/// How many straight steps draw an arc within the display tolerance.
fn arc_steps(radius: f64, sweep: f64) -> usize {
    let max_step = if radius > DISPLAY_TOLERANCE {
        2.0 * (1.0 - DISPLAY_TOLERANCE / radius).acos()
    } else {
        TAU / 8.0
    };
    ((sweep.abs() / max_step).ceil() as usize).clamp(2, MAX_CURVE_STEPS)
}

// --- Toolpaths ---------------------------------------------------------------------------------

/// What kind of move a run of toolpath points is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveKind {
    /// Feed moves that cut sideways (lines and arcs).
    Cut,
    /// Feed moves straight down (or up) only.
    Plunge,
    Rapid,
}

/// A run of connected points of one kind, in 3D.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolpathRun {
    pub kind: MoveKind,
    pub points: Vec<[f64; 3]>,
}

/// A toolpath ready to draw: runs of cut, plunge and rapid moves, arcs as short steps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolpathDisplay {
    pub operation_id: u32,
    pub runs: Vec<ToolpathRun>,
}

pub fn toolpath_display(tp: &Toolpath) -> ToolpathDisplay {
    let mut runs: Vec<ToolpathRun> = Vec::new();
    let mut at: Option<Point3> = None;
    for m in &tp.moves {
        let to = m.to();
        let Some(from) = at else {
            at = Some(to);
            continue;
        };
        let (kind, mut pts) = move_points(from, m);
        match runs.last_mut() {
            Some(run) if run.kind == kind => run.points.append(&mut pts),
            _ => {
                let mut points = vec![[from.x, from.y, from.z]];
                points.append(&mut pts);
                runs.push(ToolpathRun { kind, points });
            }
        }
        at = Some(to);
    }
    ToolpathDisplay {
        operation_id: tp.operation_id,
        runs,
    }
}

/// The kind of a move from `from`, and the points it passes through after `from` (arcs as short
/// steps within the display tolerance, ending exactly at the move's end). Shared by the drawn
/// toolpath and the playback, so the two always agree.
fn move_points(from: Point3, m: &Move) -> (MoveKind, Vec<[f64; 3]>) {
    let to = m.to();
    match *m {
        Move::Rapid { .. } => (MoveKind::Rapid, vec![[to.x, to.y, to.z]]),
        Move::Feed { .. } => {
            let sideways = (to.x - from.x).hypot(to.y - from.y) > 1e-9;
            let kind = if sideways {
                MoveKind::Cut
            } else {
                MoveKind::Plunge
            };
            (kind, vec![[to.x, to.y, to.z]])
        }
        Move::Arc {
            center_x,
            center_y,
            ccw,
            ..
        } => {
            let c = Point::new(center_x, center_y);
            let (radius, a0, sweep) = arc_of(from, to, c, ccw);
            let mut flat = Vec::new();
            push_arc(&mut flat, c, radius, a0, sweep);
            flat.pop(); // replaced by the exact end point
            // A helix (an arc that goes down as it turns) descends evenly along the arc.
            let n = flat.len();
            let mut pts: Vec<[f64; 3]> = flat
                .iter()
                .enumerate()
                .skip(1)
                .map(|(i, p)| [p[0], p[1], from.z + (to.z - from.z) * i as f64 / n as f64])
                .collect();
            pts.push([to.x, to.y, to.z]);
            (MoveKind::Cut, pts)
        }
    }
}

/// Radius, start angle (radians) and signed sweep (radians, negative = clockwise) of an arc move.
/// Equal start and end points mean a full circle.
fn arc_of(from: Point3, to: Point3, c: Point, ccw: bool) -> (f64, f64, f64) {
    let p0 = Point::new(from.x, from.y);
    let p1 = Point::new(to.x, to.y);
    let radius = c.distance_to(p0);
    let ccw_sweep = arc_sweep_deg(angle_deg(c, p0), angle_deg(c, p1)).to_radians();
    let sweep = if ccw { ccw_sweep } else { ccw_sweep - TAU };
    (radius, angle_deg(c, p0).to_radians(), sweep)
}

// --- Cut width ---------------------------------------------------------------------------------

/// The band the cutter sweeps along one playback step, as flat triangles for the screen. Empty
/// for rapids and plunges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CutWidthStep {
    /// Triangles, three corners each: [x0, y0, z0, x1, y1, z1, x2, y2, z2, ...]. In order: a disc
    /// at the start, then for each segment of the step's points its strip (2 triangles) and the
    /// rounded join at its end, then a disc at the end.
    pub triangles: Vec<f64>,
    /// For each segment of the step's points (points[k] → points[k + 1]), the index of the
    /// triangle its strip starts at. A segment with no length has no strip (its index is where
    /// the next one starts), so the band can be revealed up to any segment as the tool moves.
    pub segment_starts: Vec<usize>,
}

/// The cut width of every step of `pb` (the same steps, in the same order).
pub fn cut_width_steps(pb: &Playback) -> Vec<CutWidthStep> {
    pb.steps
        .iter()
        .map(|st| {
            if st.kind == MoveKind::Cut && st.tool_diameter > 0.0 {
                sweep_step(&st.points, st.tool_diameter / 2.0)
            } else {
                CutWidthStep {
                    triangles: Vec::new(),
                    segment_starts: vec![0; st.points.len().saturating_sub(1)],
                }
            }
        })
        .collect()
}

fn push_tri(tris: &mut Vec<f64>, a: [f64; 3], b: [f64; 3], c: [f64; 3]) {
    tris.extend_from_slice(&a);
    tris.extend_from_slice(&b);
    tris.extend_from_slice(&c);
}

/// A fan of triangles round `c` (radius `r`, at its height) from angle `start` through `sweep`.
fn push_fan(tris: &mut Vec<f64>, c: [f64; 3], r: f64, start: f64, sweep: f64) {
    let steps = arc_steps(r, sweep);
    let at = |a: f64| [c[0] + r * a.cos(), c[1] + r * a.sin(), c[2]];
    for i in 0..steps {
        let a0 = start + sweep * i as f64 / steps as f64;
        let a1 = start + sweep * (i + 1) as f64 / steps as f64;
        push_tri(tris, c, at(a0), at(a1));
    }
}

/// The swept band along `pts` for a cutter of radius `r`: strips either side of the path, rounded
/// joins on the outside of bends, and round ends.
fn sweep_step(pts: &[[f64; 3]], r: f64) -> CutWidthStep {
    use std::f64::consts::{FRAC_PI_2, PI};
    let mut tris: Vec<f64> = Vec::new();
    let mut segment_starts = Vec::with_capacity(pts.len().saturating_sub(1));
    let count = |t: &Vec<f64>| t.len() / 9;
    let long = |a: [f64; 3], b: [f64; 3]| (b[0] - a[0]).hypot(b[1] - a[1]) > 1e-9;
    let dir = |a: [f64; 3], b: [f64; 3]| (b[1] - a[1]).atan2(b[0] - a[0]);
    if let Some(&first) = pts.first() {
        push_fan(&mut tris, first, r, 0.0, TAU);
    }
    for k in 0..pts.len().saturating_sub(1) {
        segment_starts.push(count(&tris));
        let (a, b) = (pts[k], pts[k + 1]);
        if !long(a, b) {
            continue;
        }
        let d = dir(a, b);
        let (nx, ny) = (-d.sin() * r, d.cos() * r); // left of the direction of travel
        push_tri(
            &mut tris,
            [a[0] + nx, a[1] + ny, a[2]],
            [a[0] - nx, a[1] - ny, a[2]],
            [b[0] + nx, b[1] + ny, b[2]],
        );
        push_tri(
            &mut tris,
            [b[0] + nx, b[1] + ny, b[2]],
            [a[0] - nx, a[1] - ny, a[2]],
            [b[0] - nx, b[1] - ny, b[2]],
        );
        // Rounded join on the outside of the bend at b, towards the next segment with length.
        if let Some(next) = (k + 1..pts.len() - 1).find(|&m| long(pts[m], pts[m + 1])) {
            let d2 = dir(pts[next], pts[next + 1]);
            let turn = (d2 - d + PI).rem_euclid(TAU) - PI;
            if turn.abs() > 1e-9 {
                // Turning left the outside is on the right, and the other way round.
                let start = if turn > 0.0 {
                    d - FRAC_PI_2
                } else {
                    d + FRAC_PI_2
                };
                push_fan(&mut tris, b, r, start, turn);
            }
        }
    }
    if pts.len() > 1 {
        push_fan(&mut tris, pts[pts.len() - 1], r, 0.0, TAU);
    }
    CutWidthStep {
        triangles: tris,
        segment_starts,
    }
}

// --- Toolpath playback -------------------------------------------------------------------------

/// Rapid rate used to estimate playback time until machine settings exist (mm/min).
pub const ASSUMED_RAPID_RATE: f64 = 5000.0;

/// One move of the playback: where the tool goes and how long it takes at its feed rate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaybackStep {
    pub operation_id: u32,
    /// The move's number within its operation's toolpath (1 = first), as in safety messages.
    pub move_number: usize,
    pub kind: MoveKind,
    /// Feed rate in mm/min; `None` for rapids.
    pub feed: Option<f64>,
    pub tool_diameter: f64,
    /// From the start of the move to its end (arcs as short steps, as drawn).
    pub points: Vec<[f64; 3]>,
    /// Seconds from the start of the playback to the start of this move.
    pub start_s: f64,
    pub duration_s: f64,
}

/// Every shown toolpath one after another, for the tool playback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playback {
    pub steps: Vec<PlaybackStep>,
    pub total_s: f64,
    /// The cutter each operation uses, to draw it in 3D.
    pub tools: Vec<PlaybackTool>,
}

/// The cutter of one operation for the 3D playback: its outline turned round its axis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaybackTool {
    pub operation_id: u32,
    /// From the tip on the axis, out and up the side, to the shank top back on the axis.
    /// x = radius, y = height above the tip, in mm (see [`crate::Tool::profile`]).
    pub profile: Vec<crate::geometry::Point>,
    /// Height where the flutes end and the shank starts.
    pub flute_length: f64,
}

/// Builds the playback of `toolpaths` (each with its tool diameter), in order. Each operation's
/// first move starts where the previous one ended; the very first move is placed directly (0 s).
/// Times use the true move lengths (arc length for arcs) at their feed rates, and
/// `ASSUMED_RAPID_RATE` for rapids.
pub fn playback(toolpaths: &[(&Toolpath, f64)]) -> Playback {
    let mut steps = Vec::new();
    let mut t = 0.0;
    let mut at: Option<Point3> = None;
    for (tp, tool_diameter) in toolpaths {
        for (i, m) in tp.moves.iter().enumerate() {
            let to = m.to();
            let from = at.unwrap_or(to);
            let (kind, mut pts) = move_points(from, m);
            let (length, feed) = match *m {
                Move::Rapid { .. } => (dist3(from, to), None),
                Move::Feed { feed, .. } => (dist3(from, to), Some(feed)),
                Move::Arc {
                    center_x,
                    center_y,
                    ccw,
                    feed,
                    ..
                } => {
                    let (radius, _, sweep) = arc_of(from, to, Point::new(center_x, center_y), ccw);
                    // True length, including any descent (a helix).
                    let length = if at.is_some() {
                        (radius * sweep.abs()).hypot(to.z - from.z)
                    } else {
                        0.0
                    };
                    (length, Some(feed))
                }
            };
            let rate = feed.unwrap_or(ASSUMED_RAPID_RATE);
            let duration_s = if rate > 0.0 {
                length / rate * 60.0
            } else {
                0.0
            };
            let mut points = vec![[from.x, from.y, from.z]];
            points.append(&mut pts);
            steps.push(PlaybackStep {
                operation_id: tp.operation_id,
                move_number: i + 1,
                kind,
                feed,
                tool_diameter: *tool_diameter,
                points,
                start_s: t,
                duration_s,
            });
            t += duration_s;
            at = Some(to);
        }
    }
    Playback {
        steps,
        total_s: t,
        tools: Vec::new(),
    }
}

fn dist3(a: Point3, b: Point3) -> f64 {
    ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt()
}
