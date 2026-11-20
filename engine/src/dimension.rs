//! Dimensions (spec 5.3): measure only, never machined, never exported. Decided 2026-11-20:
//!
//! - **Their own list**, not on drawing layers: the document's `dimensions`, shown under a
//!   Dimensions heading, drawn in one fixed colour, shown or hidden together.
//! - **Attached to the geometry.** A dimension stores *what* it measures (a shape's end, midpoint
//!   or centre; a straight segment; a circle or arc) and *where it sits* relative to that. Its
//!   value and position are worked out again from the current drawing every time, so it follows
//!   moves, scales, rotations and edits, and moving a part carries its dimensions along.
//! - When what it measures is deleted or replaced (Explode, Join, Break give new ids, or a
//!   polyline loses that segment), the dimension is deleted in the same undo step.
//! - **Placing:** the kind comes from the clicks (see `Project::dimension_click`); the defining
//!   clicks must land on shapes; the last click (where it sits) is free, never snapped.
//!   Dragging a dimension moves where it sits the same way.
//! - **Size on screen:** text and arrowheads keep the same size in pixels at every zoom.

use std::f64::consts::{PI, TAU};

use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::draw::{SnapKind, snap_features};
use crate::geometry::{
    Point, Shape, angle_deg, bulge_arc, cross, distance_to_segment, dot, on_circle,
    rectangle_corners, sub,
};

/// Colour dimensions are drawn in (one fixed colour, apart from the layer colours).
pub const DIMENSION_COLOR: &str = "#8fb4d9";
/// Sizes on screen, in pixels (scaled to mm by the current zoom).
const TEXT_PX: f64 = 12.0;
const ARROW_LENGTH_PX: f64 = 9.0;
const ARROW_HALF_WIDTH_PX: f64 = 2.5;
/// Extension lines start this far from the measured point and run this far past the
/// dimension line.
const EXTENSION_GAP_PX: f64 = 4.0;
const EXTENSION_BEYOND_PX: f64 = 6.0;
/// Space between a line and its label.
const LABEL_GAP_PX: f64 = 4.0;
/// Rough width of one character as a fraction of the text height (for the label's box).
const CHAR_WIDTH: f64 = 0.6;
/// Points closer than this (mm) count as the same point.
const SAME_POINT: f64 = 1e-9;

// ---------------------------------------------------------------------------------------------
// What a dimension measures

/// A point on a shape: its `n`th snap point of `kind` (an end, midpoint or centre), as the snap
/// points list them. `shape` is the shape's type when picked: if it changes, the point is gone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    pub id: u32,
    pub shape: String,
    pub kind: SnapKind,
    pub n: usize,
}

/// One segment of a shape: a line, an arc or circle, a polyline segment, or a rectangle side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SegmentRef {
    pub id: u32,
    pub shape: String,
    pub index: usize,
}

/// What a dimension click landed on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Target {
    Point(Anchor),
    /// A straight segment.
    Line(SegmentRef),
    /// A circle, an arc, or an arc segment of a polyline.
    Circle(SegmentRef),
}

/// A dimension as stored: what it measures and where it sits relative to that.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum DimensionDef {
    /// The distance between two points, with the dimension line `offset` mm to the left of the
    /// direction from `from` to `to` (negative: to the right).
    Aligned {
        from: Anchor,
        to: Anchor,
        offset: f64,
    },
    /// A straight segment's length, placed as `Aligned`.
    Length { line: SegmentRef, offset: f64 },
    /// The distance between two parallel lines: from the point `along` (0 to 1) of the way along
    /// `a`, square across to `b`, placed as `Aligned`.
    Parallel {
        a: SegmentRef,
        along: f64,
        b: SegmentRef,
        offset: f64,
    },
    /// A circle's or arc's radius (`diameter` false) or diameter, with the value `distance` mm
    /// from the centre in the direction `angle` (degrees).
    Circular {
        circle: SegmentRef,
        diameter: bool,
        angle: f64,
        distance: f64,
    },
    /// The angle between two lines (extended to where they cross), on the side of each line
    /// chosen by `a_reversed` / `b_reversed`, with its arc `radius` mm from the crossing.
    Angle {
        a: SegmentRef,
        a_reversed: bool,
        b: SegmentRef,
        b_reversed: bool,
        radius: f64,
    },
}

/// A dimension in the document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DimensionObject {
    /// Unique among the document's objects and dimensions.
    pub id: u32,
    pub def: DimensionDef,
}

/// The kinds, for the tree and Properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DimensionKind {
    Aligned,
    Radius,
    Diameter,
    Angle,
}

/// A dimension worked out from the current drawing: where it is and its value (mm, or degrees
/// for an angle).
#[derive(Debug, Clone, PartialEq)]
pub enum Dimension {
    /// From `from` to `to`, the dimension line `offset` mm to the left of that direction.
    Aligned {
        from: Point,
        to: Point,
        offset: f64,
        value: f64,
    },
    Radius {
        center: Point,
        radius: f64,
        at: Point,
        value: f64,
    },
    Diameter {
        center: Point,
        radius: f64,
        at: Point,
        value: f64,
    },
    /// Anticlockwise from the direction of `start` to that of `end`, its arc at the distance of
    /// `start` from `vertex`.
    Angle {
        vertex: Point,
        start: Point,
        end: Point,
        value: f64,
    },
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
}

// ---------------------------------------------------------------------------------------------
// Finding the measured features in the drawing

/// One segment of a shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Segment {
    Straight(Point, Point),
    /// An arc (from `a` to `b` with `bulge`) or, with `full`, a whole circle.
    Round {
        center: Point,
        radius: f64,
        full: bool,
        a: Point,
        b: Point,
        bulge: f64,
    },
}

impl Segment {
    fn distance(&self, p: Point) -> f64 {
        match *self {
            Segment::Straight(a, b) => distance_to_segment(p, a, b, 0.0),
            Segment::Round {
                center,
                radius,
                full: true,
                ..
            } => (p.distance_to(center) - radius).abs(),
            Segment::Round { a, b, bulge, .. } => distance_to_segment(p, a, b, bulge),
        }
    }
}

/// A shape's segments, in a fixed order (the index a `SegmentRef` uses).
pub fn segments(shape: &Shape) -> Vec<Segment> {
    match shape {
        Shape::Line { start, end } => vec![Segment::Straight(*start, *end)],
        Shape::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let a = on_circle(*center, *radius, start_angle.to_radians());
            let b = on_circle(*center, *radius, end_angle.to_radians());
            let sweep = crate::geometry::arc_sweep_deg(*start_angle, *end_angle).to_radians();
            vec![Segment::Round {
                center: *center,
                radius: *radius,
                full: false,
                a,
                b,
                bulge: crate::geometry::bulge_from_sweep(sweep),
            }]
        }
        Shape::Circle { center, radius } => vec![Segment::Round {
            center: *center,
            radius: *radius,
            full: true,
            a: *center,
            b: *center,
            bulge: 0.0,
        }],
        Shape::Rectangle {
            lower_left,
            width,
            height,
            rotation,
        } => {
            let c = rectangle_corners(*lower_left, *width, *height, *rotation);
            (0..4)
                .map(|i| Segment::Straight(c[i], c[(i + 1) % 4]))
                .collect()
        }
        Shape::Polyline(pl) => {
            let v = &pl.vertices;
            let n = v.len();
            let count = if pl.closed { n } else { n.saturating_sub(1) };
            (0..count)
                .map(|i| {
                    let (a, b) = (v[i].point(), v[(i + 1) % n].point());
                    match bulge_arc(a, b, v[i].bulge) {
                        Some(arc) => Segment::Round {
                            center: arc.center,
                            radius: arc.radius,
                            full: false,
                            a,
                            b,
                            bulge: v[i].bulge,
                        },
                        None => Segment::Straight(a, b),
                    }
                })
                .collect()
        }
        Shape::Points { .. } => Vec::new(),
    }
}

/// The shape with id `id`, if it is still the same type.
fn shape<'d>(doc: &'d Document, id: u32, type_name: &str) -> Option<&'d Shape> {
    doc.objects()
        .find(|o| o.id == id)
        .map(|o| &o.shape)
        .filter(|s| s.type_name() == type_name)
}

/// Where an anchor is now.
pub fn anchor_point(doc: &Document, a: &Anchor) -> Option<Point> {
    let s = shape(doc, a.id, &a.shape)?;
    snap_features(s)
        .into_iter()
        .filter(|(_, kind)| *kind == a.kind)
        .nth(a.n)
        .map(|(p, _)| p)
}

/// A segment as it is now.
fn segment(doc: &Document, r: &SegmentRef) -> Option<Segment> {
    segments(shape(doc, r.id, &r.shape)?).get(r.index).copied()
}

fn line(doc: &Document, r: &SegmentRef) -> Option<(Point, Point)> {
    match segment(doc, r)? {
        Segment::Straight(a, b) if a.distance_to(b) > SAME_POINT => Some((a, b)),
        _ => None,
    }
}

fn circle(doc: &Document, r: &SegmentRef) -> Option<(Point, f64)> {
    match segment(doc, r)? {
        Segment::Round { center, radius, .. } if radius > SAME_POINT => Some((center, radius)),
        _ => None,
    }
}

/// The anchors of a shape: every end, midpoint and centre, with where they are.
pub fn anchors(id: u32, s: &Shape) -> Vec<(Point, Anchor)> {
    let mut count = std::collections::HashMap::new();
    snap_features(s)
        .into_iter()
        .map(|(p, kind)| {
            let n = count.entry(kind).or_insert(0usize);
            let a = Anchor {
                id,
                shape: s.type_name().to_string(),
                kind,
                n: *n,
            };
            *n += 1;
            (p, a)
        })
        .collect()
}

/// The segment of shape `id` nearest to `click` that is round (`round`) or straight, with its
/// distance from the click.
pub fn nearest_segment(id: u32, s: &Shape, click: Point, round: bool) -> Option<(f64, SegmentRef)> {
    segments(s)
        .iter()
        .enumerate()
        .filter(|(_, seg)| matches!(seg, Segment::Round { .. }) == round)
        .map(|(index, seg)| {
            (
                seg.distance(click),
                SegmentRef {
                    id,
                    shape: s.type_name().to_string(),
                    index,
                },
            )
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
}

// ---------------------------------------------------------------------------------------------
// Working a dimension out from the drawing

/// The two ends an aligned-style dimension measures between.
fn measured_ends(doc: &Document, def: &DimensionDef) -> Option<(Point, Point)> {
    match def {
        DimensionDef::Aligned { from, to, .. } => {
            Some((anchor_point(doc, from)?, anchor_point(doc, to)?))
        }
        DimensionDef::Length { line: l, .. } => line(doc, l),
        DimensionDef::Parallel { a, along, b, .. } => {
            let (a0, a1) = line(doc, a)?;
            let from = Point::new(a0.x + (a1.x - a0.x) * along, a0.y + (a1.y - a0.y) * along);
            let (b0, b1) = line(doc, b)?;
            Some((from, foot(from, b0, b1)))
        }
        _ => None,
    }
}

/// The point on the (extended) line through `a` and `b` nearest to `p`.
fn foot(p: Point, a: Point, b: Point) -> Point {
    let d = sub(b, a);
    let t = dot(sub(p, a), d) / dot(d, d);
    Point::new(a.x + d.x * t, a.y + d.y * t)
}

/// Where two (extended) lines cross, if they aren't parallel.
fn crossing(a: (Point, Point), b: (Point, Point)) -> Option<Point> {
    let (d1, d2) = (sub(a.1, a.0), sub(b.1, b.0));
    let denom = cross(d1, d2);
    let scale = d1.x.hypot(d1.y) * d2.x.hypot(d2.y);
    if denom.abs() <= 1e-12 * scale {
        return None;
    }
    let t = cross(sub(b.0, a.0), d2) / denom;
    Some(Point::new(a.0.x + d1.x * t, a.0.y + d1.y * t))
}

fn unit(d: Point) -> Point {
    let len = d.x.hypot(d.y);
    if len > 0.0 {
        Point::new(d.x / len, d.y / len)
    } else {
        Point::new(1.0, 0.0)
    }
}

/// A dimension as it is now in `doc`, or `None` if what it measures is gone.
pub fn resolve(doc: &Document, def: &DimensionDef) -> Option<Dimension> {
    match def {
        DimensionDef::Aligned { offset, .. }
        | DimensionDef::Length { offset, .. }
        | DimensionDef::Parallel { offset, .. } => {
            let (from, to) = measured_ends(doc, def)?;
            let value = from.distance_to(to);
            (value > SAME_POINT).then_some(Dimension::Aligned {
                from,
                to,
                offset: *offset,
                value,
            })
        }
        DimensionDef::Circular {
            circle: c,
            diameter,
            angle,
            distance,
        } => {
            let (center, radius) = circle(doc, c)?;
            let at = on_circle(center, *distance, angle.to_radians());
            Some(if *diameter {
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
        DimensionDef::Angle {
            a,
            a_reversed,
            b,
            b_reversed,
            radius,
        } => {
            let (la, lb) = (line(doc, a)?, line(doc, b)?);
            let vertex = crossing(la, lb)?;
            let ray = |l: (Point, Point), rev: bool| {
                let d = unit(sub(l.1, l.0));
                if rev { Point::new(-d.x, -d.y) } else { d }
            };
            let (ra, rb) = (ray(la, *a_reversed), ray(lb, *b_reversed));
            // The angle between two rays is the one under 180°: start from whichever ray turns
            // anticlockwise into the other through it.
            let (start, end) = if cross(ra, rb) >= 0.0 {
                (ra, rb)
            } else {
                (rb, ra)
            };
            let value = dot(ra, rb).clamp(-1.0, 1.0).acos().to_degrees();
            let r = radius.max(SAME_POINT * 10.0);
            Some(Dimension::Angle {
                vertex,
                start: plus(vertex, start, r),
                end: plus(vertex, end, r),
                value,
            })
        }
    }
}

/// Places a dimension so it sits at `at` (the last click, or where it is dragged to), keeping
/// what it measures. `None` if what it measures is gone.
pub fn place(doc: &Document, def: &DimensionDef, at: Point) -> Option<DimensionDef> {
    let ends = measured_ends(doc, def);
    let mut def = def.clone();
    match &mut def {
        DimensionDef::Aligned { offset, .. }
        | DimensionDef::Length { offset, .. }
        | DimensionDef::Parallel { offset, .. } => {
            let (from, to) = ends?;
            let u = unit(sub(to, from));
            *offset = dot(sub(at, from), Point::new(-u.y, u.x));
        }
        DimensionDef::Circular {
            circle: c,
            angle,
            distance,
            ..
        } => {
            let (center, _) = circle(doc, c)?;
            *angle = angle_deg(center, at);
            *distance = center.distance_to(at);
        }
        DimensionDef::Angle {
            a,
            a_reversed,
            b,
            b_reversed,
            radius,
        } => {
            let (la, lb) = (line(doc, a)?, line(doc, b)?);
            let vertex = crossing(la, lb)?;
            let dir = |d: Point| d.y.atan2(d.x).rem_euclid(TAU);
            let (a1, a2) = (dir(sub(la.1, la.0)), dir(sub(lb.1, lb.0)));
            // The four rays out of the crossing, sorted anticlockwise: `at` falls between two
            // neighbours, one from each line, which bound the angle it chose.
            let mut rays = [
                (a1, true, false),
                ((a1 + PI) % TAU, true, true),
                (a2, false, false),
                ((a2 + PI) % TAU, false, true),
            ];
            rays.sort_by(|x, y| x.0.total_cmp(&y.0));
            let q = dir(sub(at, vertex));
            let i = rays.iter().rposition(|r| r.0 <= q).unwrap_or(3);
            for (_, is_a, rev) in [rays[i], rays[(i + 1) % 4]] {
                if is_a {
                    *a_reversed = rev;
                } else {
                    *b_reversed = rev;
                }
            }
            *radius = vertex.distance_to(at).max(SAME_POINT * 10.0);
        }
    }
    Some(def)
}

/// Whether the clicks so far (each the thing it picked, or `None` for the free last click) make
/// a whole dimension: a circle/arc then where it goes; a line then where its length goes; two
/// lines then where the angle or distance goes; two points then where it goes.
pub fn complete(targets: &[Option<&Target>]) -> bool {
    use Target::{Circle, Line, Point as P};
    matches!(
        targets,
        [Some(Circle(_)), None]
            | [Some(Line(_)), None]
            | [Some(Line(_)), Some(Line(_)), None]
            | [Some(P(_)), Some(P(_)), None]
    )
}

/// The dimension the clicked targets make (before it is placed), or `None` if they don't make
/// one. Two lines make an angle, or the distance between them if they are parallel (measured
/// from where `first_click` falls on the first).
pub fn from_targets(
    doc: &Document,
    targets: &[Target],
    first_click: Point,
) -> Option<DimensionDef> {
    let def = match targets {
        [Target::Circle(c)] => {
            let full = matches!(segment(doc, c)?, Segment::Round { full: true, .. });
            DimensionDef::Circular {
                circle: c.clone(),
                diameter: full,
                angle: 45.0,
                distance: 0.0,
            }
        }
        [Target::Line(l)] => DimensionDef::Length {
            line: l.clone(),
            offset: 0.0,
        },
        [Target::Line(a), Target::Line(b)] => {
            let (la, lb) = (line(doc, a)?, line(doc, b)?);
            if crossing(la, lb).is_some() {
                DimensionDef::Angle {
                    a: a.clone(),
                    a_reversed: false,
                    b: b.clone(),
                    b_reversed: false,
                    radius: 1.0,
                }
            } else {
                let d = sub(la.1, la.0);
                let along = (dot(sub(first_click, la.0), d) / dot(d, d)).clamp(0.0, 1.0);
                DimensionDef::Parallel {
                    a: a.clone(),
                    along,
                    b: b.clone(),
                    offset: 0.0,
                }
            }
        }
        [Target::Point(from), Target::Point(to)] => DimensionDef::Aligned {
            from: from.clone(),
            to: to.clone(),
            offset: 0.0,
        },
        _ => return None,
    };
    // E.g. two points on top of each other, or lines on top of each other: nothing to measure.
    resolve(doc, &def)?;
    Some(def)
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
    /// Text height in mm (at the zoom it was laid out for).
    pub height: f64,
}

impl Label {
    /// The corners of the text's box (estimated from the number of characters).
    pub fn corners(&self) -> [[f64; 2]; 4] {
        let w = self.half_width();
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

/// Lays a dimension out for a zoom of `mm_per_px` millimetres per screen pixel.
pub fn layout(d: &Dimension, mm_per_px: f64) -> Layout {
    let px = mm_per_px;
    let text = d.text();
    let height = TEXT_PX * px;
    let gap = LABEL_GAP_PX * px;
    // An arrowhead with its tip at `tip`, pointing along the unit direction `towards`.
    let arrow = |tip: Point, towards: Point| {
        let back = plus(tip, towards, -ARROW_LENGTH_PX * px);
        let n = Point::new(-towards.y, towards.x);
        vec![
            xy(tip),
            xy(plus(back, n, ARROW_HALF_WIDTH_PX * px)),
            xy(plus(back, n, -ARROW_HALF_WIDTH_PX * px)),
            xy(tip),
        ]
    };
    // A horizontal label beside `p`, pushed out along the unit direction `out` until its
    // nearest edge clears `p` by the gap.
    let label_beyond = |text: String, p: Point, out: Point| {
        let mut label = Label {
            text,
            at: xy(p),
            angle: 0.0,
            height,
        };
        let reach = label.half_width() * out.x.abs() + height / 2.0 * out.y.abs();
        label.at = xy(plus(p, out, reach + gap));
        label
    };
    match *d {
        Dimension::Aligned {
            from, to, offset, ..
        } => {
            let u = unit(sub(to, from));
            let n = Point::new(-u.y, u.x);
            let side = if offset < 0.0 { -1.0 } else { 1.0 };
            let (a, b) = (plus(from, n, offset), plus(to, n, offset));
            let mut lines = Vec::new();
            // Extension lines from just off each point to just past the dimension line.
            if offset.abs() > EXTENSION_GAP_PX * px {
                for p in [from, to] {
                    lines.push(vec![
                        xy(plus(p, n, side * EXTENSION_GAP_PX * px)),
                        xy(plus(p, n, offset + side * EXTENSION_BEYOND_PX * px)),
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
                    at: xy(plus(mid, n, side * (gap + height / 2.0))),
                    angle: readable(u.y.atan2(u.x).to_degrees()),
                    height,
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
