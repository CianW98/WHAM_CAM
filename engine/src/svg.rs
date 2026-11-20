//! Reads 2D shapes from SVG text (spec 5.1). The only engine file that uses the `roxmltree`
//! (XML) and `svgtypes` (SVG path data, transforms, lengths) libraries (CLAUDE.md: one adapter
//! file per library). Takes the file's text: the engine never touches the file system.
//!
//! Decided 2026-11-20:
//! - Shapes stay exact where they can: `<circle>` → Circle, `<rect>` → PolyRectangle, circular
//!   arcs in paths → true arcs. Bézier curves, ellipses and elliptical arcs become arcs and lines
//!   within the import's curve tolerance (as DXF splines do).
//! - Size: when the file gives its real size (`width="210mm"` with a `viewBox`), the scale is
//!   exact; otherwise the caller says how many mm one SVG unit is (the import asks the user).
//! - The drawing is flipped the right way up (SVG measures Y downwards) with the page's
//!   bottom-left corner at 0,0.
//! - Inkscape layers become layers of the same name; anything outside one goes on the default
//!   layer (named after the file).
//! - Only outlines are read: fills, strokes and styles are ignored. Text, images, `<use>` copies
//!   and nested `<svg>` are skipped and listed.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use svgtypes::{Length, LengthUnit, PathParser, PathSegment, PointsParser, ViewBox};

use crate::curve::{self, Curve, Spline};
use crate::geometry::{
    Point, Polyline, Shape, Transform, Vertex, bulge_from_sweep, svg_arc_center,
};

const SVG_NS: &str = "http://www.w3.org/2000/svg";
const INKSCAPE_NS: &str = "http://www.inkscape.org/namespaces/inkscape";
/// CSS: 96 px to the inch, the SVG user unit when a file has no viewBox.
const MM_PER_PX: f64 = 25.4 / 96.0;

/// How to read an SVG file.
#[derive(Debug, Clone, PartialEq)]
pub struct SvgOptions {
    /// Millimetres per SVG unit, used only when the file doesn't give its real size.
    pub mm_per_unit: f64,
    /// Curves become arcs and lines within this distance (mm) of the true curve.
    pub curve_tolerance: f64,
    /// Layer for shapes that aren't in an Inkscape layer.
    pub default_layer: String,
}

/// Everything read from an SVG file.
#[derive(Debug, Clone, PartialEq)]
pub struct SvgDrawing {
    /// (layer name, shape), in file order, in mm.
    pub shapes: Vec<(String, Shape)>,
    /// Whether the file gave its real size (then `SvgOptions::mm_per_unit` wasn't used).
    pub size_known: bool,
    /// Millimetres per SVG unit actually used.
    pub mm_per_unit: f64,
    /// How many curves (Béziers, ellipses, elliptical arcs) were converted into arcs and lines.
    pub curves: usize,
    /// Elements not imported, by description, with counts.
    pub skipped: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SvgError {
    /// Not readable as XML, or the top element isn't `<svg>`.
    NotSvg(String),
    /// The scale or curve tolerance isn't a number more than 0.
    BadOptions,
}

impl fmt::Display for SvgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SvgError::NotSvg(why) => write!(f, "This doesn't look like an SVG file ({why})."),
            SvgError::BadOptions => write!(f, "The scale and tolerance must be more than 0."),
        }
    }
}

pub fn read_svg(text: &str, options: &SvgOptions) -> Result<SvgDrawing, SvgError> {
    let positive = |v: f64| v.is_finite() && v > 0.0;
    if !positive(options.mm_per_unit) || !positive(options.curve_tolerance) {
        return Err(SvgError::BadOptions);
    }
    let doc = roxmltree::Document::parse(text).map_err(|e| SvgError::NotSvg(e.to_string()))?;
    let root = doc.root_element();
    if root.tag_name().name() != "svg" {
        return Err(SvgError::NotSvg("its top element isn't <svg>".into()));
    }
    let page = Page::of(&root);
    let mm_per_unit = page.mm_per_unit.unwrap_or(options.mm_per_unit);
    // SVG units → mm: from the viewBox origin, scaled, flipped so +Y is up, with the page's
    // bottom edge at y = 0 (or flipped at 0 if the page height isn't known).
    let to_mm = Transform::translate(-page.origin.x, -page.origin.y)
        .then(Transform::scale(mm_per_unit, -mm_per_unit))
        .then(Transform::translate(
            0.0,
            page.height.map_or(0.0, |h| h * mm_per_unit),
        ));
    let mut reader = Reader {
        tolerance: options.curve_tolerance,
        shapes: Vec::new(),
        curves: 0,
        skipped: BTreeMap::new(),
    };
    reader.children(&root, &to_mm, &options.default_layer);
    Ok(SvgDrawing {
        shapes: reader.shapes,
        size_known: page.mm_per_unit.is_some(),
        mm_per_unit,
        curves: reader.curves,
        skipped: reader.skipped,
    })
}

// ---------------------------------------------------------------------------------------------
// Page size and units

struct Page {
    /// Millimetres per SVG unit, if the file gives its real size.
    mm_per_unit: Option<f64>,
    /// Page height in SVG units, if known.
    height: Option<f64>,
    /// The viewBox's top-left corner (SVG units).
    origin: Point,
}

/// A length in real units, in mm (`None` for unit-less numbers, px, %, em and ex).
fn real_mm(len: Length) -> Option<f64> {
    let per = match len.unit {
        LengthUnit::Mm => 1.0,
        LengthUnit::Cm => 10.0,
        LengthUnit::In => 25.4,
        LengthUnit::Pt => 25.4 / 72.0,
        LengthUnit::Pc => 25.4 / 6.0,
        _ => return None,
    };
    (len.number > 0.0).then_some(len.number * per)
}

impl Page {
    fn of(root: &roxmltree::Node) -> Page {
        let length = |name: &str| root.attribute(name).and_then(|v| Length::from_str(v).ok());
        let (w, h) = (length("width"), length("height"));
        let view_box = root
            .attribute("viewBox")
            .and_then(|v| ViewBox::from_str(v).ok())
            .filter(|vb| vb.w > 0.0 && vb.h > 0.0);
        match view_box {
            Some(vb) => {
                let sx = w.and_then(real_mm).map(|mm| mm / vb.w);
                let sy = h.and_then(real_mm).map(|mm| mm / vb.h);
                Page {
                    // Different scales: the smaller fits the page (SVG's default "meet").
                    mm_per_unit: match (sx, sy) {
                        (Some(a), Some(b)) => Some(a.min(b)),
                        (a, b) => a.or(b),
                    },
                    height: Some(vb.h),
                    origin: Point::new(vb.x, vb.y),
                }
            }
            None => {
                // No viewBox: one SVG unit is one CSS px. A real width or height says so.
                let real = w.and_then(real_mm).is_some() || h.and_then(real_mm).is_some();
                let height = h.and_then(|h| match h.unit {
                    LengthUnit::None | LengthUnit::Px => Some(h.number),
                    _ => real_mm(h).map(|mm| mm / MM_PER_PX),
                });
                Page {
                    mm_per_unit: real.then_some(MM_PER_PX),
                    height,
                    origin: Point::new(0.0, 0.0),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Walking the elements

struct Reader {
    tolerance: f64,
    shapes: Vec<(String, Shape)>,
    curves: usize,
    skipped: BTreeMap<String, usize>,
}

/// The transform of an element (SVG's matrix(a b c d e f) maps x' = a·x + c·y + e,
/// y' = b·x + d·y + f).
fn own_transform(node: &roxmltree::Node) -> Option<Transform> {
    let Some(text) = node.attribute("transform") else {
        return Some(Transform::IDENTITY);
    };
    let t = svgtypes::Transform::from_str(text).ok()?;
    Some(Transform {
        a: t.a,
        b: t.c,
        c: t.b,
        d: t.d,
        e: t.e,
        f: t.f,
    })
}

/// Hidden with `display="none"` or `style="display:none"` (as Inkscape hides layers).
fn hidden(node: &roxmltree::Node) -> bool {
    if node.attribute("display") == Some("none") {
        return true;
    }
    node.attribute("style").is_some_and(|style| {
        style.split(';').any(|decl| {
            let mut kv = decl.splitn(2, ':');
            let key = kv.next().map(str::trim);
            let value = kv.next().map(str::trim);
            key == Some("display") && value == Some("none")
        })
    })
}

/// A number attribute in SVG units (unit-less or px; absolute units converted as CSS does).
fn number(node: &roxmltree::Node, name: &str) -> Option<f64> {
    let len = Length::from_str(node.attribute(name)?).ok()?;
    match len.unit {
        LengthUnit::None | LengthUnit::Px => Some(len.number),
        _ => real_mm(len).map(|mm| mm / MM_PER_PX),
    }
}

impl Reader {
    fn skip(&mut self, what: impl Into<String>) {
        *self.skipped.entry(what.into()).or_insert(0) += 1;
    }

    fn children(&mut self, node: &roxmltree::Node, t: &Transform, layer: &str) {
        for child in node.children().filter(|c| c.is_element()) {
            self.element(&child, t, layer);
        }
    }

    fn element(&mut self, node: &roxmltree::Node, parent: &Transform, layer: &str) {
        // Elements from other namespaces (e.g. Inkscape's page settings) aren't drawn.
        let ns = node.tag_name().namespace();
        if ns.is_some_and(|ns| ns != SVG_NS) || hidden(node) {
            return;
        }
        let name = node.tag_name().name();
        let Some(own) = own_transform(node) else {
            self.skip(format!("{name} (unreadable transform)"));
            return;
        };
        let t = own.then(*parent);
        let ok = match name {
            "g" | "a" | "switch" => {
                let is_layer = node.attribute((INKSCAPE_NS, "groupmode")) == Some("layer");
                let label = node
                    .attribute((INKSCAPE_NS, "label"))
                    .or_else(|| node.attribute("id"));
                let layer = match (is_layer, label) {
                    (true, Some(l)) if !l.trim().is_empty() => l.trim(),
                    _ => layer,
                };
                self.children(node, &t, layer);
                Some(())
            }
            // Not drawn themselves (definitions, styles, descriptions).
            "defs" | "symbol" | "clipPath" | "mask" | "marker" | "pattern" | "linearGradient"
            | "radialGradient" | "filter" | "style" | "script" | "metadata" | "title" | "desc" => {
                Some(())
            }
            "line" => (|| {
                let p = |x: &str, y: &str| {
                    Some(Point::new(
                        number(node, x).unwrap_or(0.0),
                        number(node, y).unwrap_or(0.0),
                    ))
                };
                let mut path = Path::new(&t, self.tolerance);
                path.move_to(p("x1", "y1")?);
                path.line_to(p("x2", "y2")?);
                self.add_paths(layer, path.finish());
                Some(())
            })(),
            "polyline" | "polygon" => {
                let pts: Vec<Point> = PointsParser::from(node.attribute("points").unwrap_or(""))
                    .map(|(x, y)| Point::new(x, y))
                    .collect();
                let mut path = Path::new(&t, self.tolerance);
                for (i, p) in pts.iter().enumerate() {
                    if i == 0 {
                        path.move_to(*p);
                    } else {
                        path.line_to(*p);
                    }
                }
                if name == "polygon" {
                    path.close();
                }
                self.add_paths(layer, path.finish());
                Some(())
            }
            "rect" => self.rect(node, &t, layer),
            "circle" => (|| {
                let r = number(node, "r")?;
                self.ellipse(node, &t, layer, r, r)
            })(),
            "ellipse" => (|| {
                let rx = number(node, "rx")?;
                let ry = number(node, "ry")?;
                self.ellipse(node, &t, layer, rx, ry)
            })(),
            "path" => self.path(node, &t, layer),
            "text" | "image" | "use" | "svg" | "foreignObject" => {
                self.skip(name);
                Some(())
            }
            other => {
                self.skip(other);
                Some(())
            }
        };
        if ok.is_none() {
            self.skip(format!("{name} (unreadable)"));
        }
    }

    fn add_paths(&mut self, layer: &str, done: PathResult) {
        self.curves += done.curves;
        for pl in done.polylines {
            self.shapes.push((layer.to_string(), Shape::Polyline(pl)));
        }
    }

    fn rect(&mut self, node: &roxmltree::Node, t: &Transform, layer: &str) -> Option<()> {
        let x = number(node, "x").unwrap_or(0.0);
        let y = number(node, "y").unwrap_or(0.0);
        let (w, h) = (number(node, "width")?, number(node, "height")?);
        if !(w > 0.0 && h > 0.0) {
            return Some(()); // SVG draws nothing
        }
        // Rounded corners: rx and ry default to each other, and at most half the side.
        let (rx, ry) = match (number(node, "rx"), number(node, "ry")) {
            (Some(a), Some(b)) => (a, b),
            (Some(a), None) | (None, Some(a)) => (a, a),
            (None, None) => (0.0, 0.0),
        };
        let (rx, ry) = (rx.clamp(0.0, w / 2.0), ry.clamp(0.0, h / 2.0));
        if rx == 0.0 || ry == 0.0 {
            // A plain rectangle stays a PolyRectangle when the transform keeps it one.
            if let Some(shape) = Shape::rectangle(Point::new(x, y), w, h, 0.0).transformed(t) {
                self.shapes.push((layer.to_string(), shape));
                return Some(());
            }
        }
        let mut path = Path::new(t, self.tolerance);
        let p = Point::new;
        path.move_to(p(x + rx, y));
        path.line_to(p(x + w - rx, y));
        path.arc_to(rx, ry, 0.0, false, true, p(x + w, y + ry));
        path.line_to(p(x + w, y + h - ry));
        path.arc_to(rx, ry, 0.0, false, true, p(x + w - rx, y + h));
        path.line_to(p(x + rx, y + h));
        path.arc_to(rx, ry, 0.0, false, true, p(x, y + h - ry));
        path.line_to(p(x, y + ry));
        path.arc_to(rx, ry, 0.0, false, true, p(x + rx, y));
        path.close();
        self.add_paths(layer, path.finish());
        Some(())
    }

    fn ellipse(
        &mut self,
        node: &roxmltree::Node,
        t: &Transform,
        layer: &str,
        rx: f64,
        ry: f64,
    ) -> Option<()> {
        let c = Point::new(
            number(node, "cx").unwrap_or(0.0),
            number(node, "cy").unwrap_or(0.0),
        );
        if !(rx > 0.0 && ry > 0.0) {
            return Some(()); // SVG draws nothing
        }
        // A circle under a transform that keeps circles round stays a true circle.
        if (rx - ry).abs() <= 1e-12 * rx
            && let Some(s) = t.uniform_scale()
        {
            self.shapes.push((
                layer.to_string(),
                Shape::Circle {
                    center: t.apply(c),
                    radius: rx * s,
                },
            ));
            return Some(());
        }
        let arc = crate::geometry::EllipticalArc {
            center: c,
            rx,
            ry,
            phi: 0.0,
            start: 0.0,
            sweep: std::f64::consts::TAU,
        };
        let pl = transformed_curve(curve::elliptical_arc(&arc), t).to_polyline(self.tolerance)?;
        self.curves += 1;
        self.shapes.push((layer.to_string(), Shape::Polyline(pl)));
        Some(())
    }

    fn path(&mut self, node: &roxmltree::Node, t: &Transform, layer: &str) -> Option<()> {
        let mut path = Path::new(t, self.tolerance);
        // Current point, subpath start, and the last control point (for smooth curves).
        let mut at = Point::new(0.0, 0.0);
        let mut start = at;
        let mut last_cubic: Option<Point> = None;
        let mut last_quad: Option<Point> = None;
        for seg in PathParser::from(node.attribute("d").unwrap_or("")) {
            // SVG draws a path up to its first error.
            let Ok(seg) = seg else { break };
            let abs_pt = |abs: bool, x: f64, y: f64| {
                if abs {
                    Point::new(x, y)
                } else {
                    Point::new(at.x + x, at.y + y)
                }
            };
            let (mut cubic, mut quad) = (None, None);
            match seg {
                PathSegment::MoveTo { abs, x, y } => {
                    at = abs_pt(abs, x, y);
                    start = at;
                    path.move_to(at);
                }
                PathSegment::LineTo { abs, x, y } => {
                    at = abs_pt(abs, x, y);
                    path.line_to(at);
                }
                PathSegment::HorizontalLineTo { abs, x } => {
                    at = Point::new(if abs { x } else { at.x + x }, at.y);
                    path.line_to(at);
                }
                PathSegment::VerticalLineTo { abs, y } => {
                    at = Point::new(at.x, if abs { y } else { at.y + y });
                    path.line_to(at);
                }
                PathSegment::CurveTo {
                    abs,
                    x1,
                    y1,
                    x2,
                    y2,
                    x,
                    y,
                } => {
                    let (c1, c2, end) =
                        (abs_pt(abs, x1, y1), abs_pt(abs, x2, y2), abs_pt(abs, x, y));
                    path.cubic_to(c1, c2, end);
                    (at, cubic) = (end, Some(c2));
                }
                PathSegment::SmoothCurveTo { abs, x2, y2, x, y } => {
                    // First control point: the last one reflected through the current point.
                    let c1 =
                        last_cubic.map_or(at, |c| Point::new(2.0 * at.x - c.x, 2.0 * at.y - c.y));
                    let (c2, end) = (abs_pt(abs, x2, y2), abs_pt(abs, x, y));
                    path.cubic_to(c1, c2, end);
                    (at, cubic) = (end, Some(c2));
                }
                PathSegment::Quadratic { abs, x1, y1, x, y } => {
                    let (c, end) = (abs_pt(abs, x1, y1), abs_pt(abs, x, y));
                    path.quad_to(c, end);
                    (at, quad) = (end, Some(c));
                }
                PathSegment::SmoothQuadratic { abs, x, y } => {
                    let c =
                        last_quad.map_or(at, |c| Point::new(2.0 * at.x - c.x, 2.0 * at.y - c.y));
                    let end = abs_pt(abs, x, y);
                    path.quad_to(c, end);
                    (at, quad) = (end, Some(c));
                }
                PathSegment::EllipticalArc {
                    abs,
                    rx,
                    ry,
                    x_axis_rotation,
                    large_arc,
                    sweep,
                    x,
                    y,
                } => {
                    let end = abs_pt(abs, x, y);
                    path.arc_to(rx, ry, x_axis_rotation, large_arc, sweep, end);
                    at = end;
                }
                PathSegment::ClosePath { .. } => {
                    path.close();
                    at = start;
                }
            }
            (last_cubic, last_quad) = (cubic, quad);
        }
        self.add_paths(layer, path.finish());
        Some(())
    }
}

/// A spline with its control points transformed (exact for any transform).
fn transformed_curve(s: Spline, t: &Transform) -> Curve {
    Curve::Spline(Spline {
        control_points: s.control_points.iter().map(|p| t.apply(*p)).collect(),
        ..s
    })
}

// ---------------------------------------------------------------------------------------------
// Building polylines from path pieces, in mm

struct PathResult {
    polylines: Vec<Polyline>,
    curves: usize,
}

/// Collects one SVG path's subpaths as polylines in mm. Points come in SVG units and are mapped
/// by the element's transform `t`.
struct Path<'t> {
    t: &'t Transform,
    tolerance: f64,
    /// The subpath being built (SVG units of its start, for closing), and its vertices in mm.
    current: Option<(Point, Vec<Vertex>)>,
    /// The current point in SVG units.
    at: Point,
    done: Vec<Polyline>,
    curves: usize,
}

impl<'t> Path<'t> {
    fn new(t: &'t Transform, tolerance: f64) -> Self {
        Path {
            t,
            tolerance,
            current: None,
            at: Point::new(0.0, 0.0),
            done: Vec::new(),
            curves: 0,
        }
    }

    fn vertex(&self, p: Point) -> Vertex {
        let q = self.t.apply(p);
        Vertex::line(q.x, q.y)
    }

    fn move_to(&mut self, p: Point) {
        self.end_subpath(false);
        self.current = Some((p, vec![self.vertex(p)]));
        self.at = p;
    }

    /// The subpath's vertices, starting one at the current point if needed (SVG lets a path
    /// carry on after Z from the subpath's start).
    fn verts(&mut self) -> &mut Vec<Vertex> {
        if self.current.is_none() {
            let v = self.vertex(self.at);
            self.current = Some((self.at, vec![v]));
        }
        &mut self.current.as_mut().expect("just set").1
    }

    fn line_to(&mut self, p: Point) {
        let v = self.vertex(p);
        self.verts().push(v);
        self.at = p;
    }

    fn arc_to(&mut self, rx: f64, ry: f64, rotation: f64, large: bool, sweep: bool, p: Point) {
        let Some(arc) = svg_arc_center(self.at, p, rx, ry, rotation, large, sweep) else {
            // A radius of 0 is a straight line; the same point twice draws nothing.
            if self.at.distance_to(p) > 0.0 {
                self.line_to(p);
            }
            return;
        };
        let circular = (arc.rx - arc.ry).abs() <= 1e-9 * arc.rx;
        if circular && let Some(_) = self.t.uniform_scale() {
            // A true arc: its bulge, turned round if the transform mirrors it (as the Y flip does).
            let flip = if self.t.mirrors() { -1.0 } else { 1.0 };
            let v = self.vertex(p);
            let verts = self.verts();
            verts.last_mut().expect("has a start").bulge = flip * bulge_from_sweep(arc.sweep);
            verts.push(v);
            self.at = p;
            return;
        }
        self.curve_to(curve::elliptical_arc(&arc), p);
    }

    fn cubic_to(&mut self, c1: Point, c2: Point, p: Point) {
        self.curve_to(curve::cubic_bezier(self.at, c1, c2, p), p);
    }

    fn quad_to(&mut self, c: Point, p: Point) {
        self.curve_to(curve::quadratic_bezier(self.at, c, p), p);
    }

    /// Appends a curve (in SVG units, from the current point to `p`) as arcs and lines.
    fn curve_to(&mut self, s: Spline, p: Point) {
        let end = self.vertex(p);
        let Some(pl) = transformed_curve(s, self.t).to_polyline(self.tolerance) else {
            // No length (all control points together): nothing to draw.
            self.at = p;
            return;
        };
        self.curves += 1;
        let verts = self.verts();
        let first_bulge = pl.vertices[0].bulge;
        verts.last_mut().expect("has a start").bulge = first_bulge;
        verts.extend(pl.vertices[1..].iter().copied());
        if pl.closed {
            // A curve that ends where it starts: the closing piece's bulge is on the last vertex.
            verts.push(end);
        } else if let Some(last) = verts.last_mut() {
            // End exactly on the curve's end point.
            (last.x, last.y) = (end.x, end.y);
        }
        self.at = p;
    }

    fn close(&mut self) {
        if let Some((start, _)) = self.current {
            self.end_subpath(true);
            self.at = start;
        }
    }

    fn end_subpath(&mut self, closed: bool) {
        let Some((_, mut verts)) = self.current.take() else {
            return;
        };
        verts.dedup_by(|b, a| a.point().distance_to(b.point()) < 1e-9 && a.bulge == 0.0);
        let mut closed = closed;
        if verts.len() > 2
            && verts[0]
                .point()
                .distance_to(verts.last().expect("len > 2").point())
                < 1e-9
        {
            // Ends on its start: the repeated point goes, the piece into it closes the shape.
            verts.pop();
            closed = true;
        }
        if verts.len() >= 2 {
            if let Some(last) = verts.last_mut()
                && !closed
            {
                last.bulge = 0.0;
            }
            self.done.push(Polyline {
                vertices: verts,
                closed,
            });
        }
    }

    fn finish(mut self) -> PathResult {
        self.end_subpath(false);
        PathResult {
            polylines: self.done,
            curves: self.curves,
        }
    }
}
