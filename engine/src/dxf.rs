//! Reads 2D drawing entities from ASCII DXF text.
//!
//! Written from Autodesk's published DXF reference. Only reading is supported, and only the
//! entities a 2.5D CAM program needs; everything else is counted as skipped so the user can be told.
//! Takes the file's text: the engine never touches the file system.

use std::collections::{BTreeMap, HashMap};
use std::f64::consts::TAU;
use std::fmt;

use crate::geometry::{Point, Polyline, Shape, Spline, Transform, Vertex};

/// Units the DXF file says it uses (header variable `$INSUNITS`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DxfUnits {
    /// The raw `$INSUNITS` code (0 = not specified).
    pub code: i32,
    /// Human-readable name, e.g. "inches".
    pub name: String,
    /// Millimetres per drawing unit, when known.
    pub mm_per_unit: Option<f64>,
}

impl DxfUnits {
    fn from_code(code: i32) -> Self {
        let (name, mm) = match code {
            0 => ("not specified", None),
            1 => ("inches", Some(25.4)),
            2 => ("feet", Some(304.8)),
            4 => ("millimetres", Some(1.0)),
            5 => ("centimetres", Some(10.0)),
            6 => ("metres", Some(1000.0)),
            8 => ("microinches", Some(0.0000254)),
            9 => ("thousandths of an inch", Some(0.0254)),
            10 => ("yards", Some(914.4)),
            13 => ("microns", Some(0.001)),
            14 => ("decimetres", Some(100.0)),
            _ => ("other", None),
        };
        Self {
            code,
            name: name.to_string(),
            mm_per_unit: mm,
        }
    }
}

/// Everything read from a DXF file.
#[derive(Debug, Clone, PartialEq)]
pub struct DxfDrawing {
    pub units: DxfUnits,
    /// (layer name, shape), in file order.
    pub shapes: Vec<(String, Shape)>,
    /// Entities that were not imported, by description, with counts.
    pub skipped: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DxfError {
    Binary,
    Empty,
    Malformed { line: usize, reason: String },
}

impl fmt::Display for DxfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DxfError::Binary => write!(
                f,
                "This is a binary DXF file. Only ASCII (text) DXF files can be imported; re-save it as ASCII DXF."
            ),
            DxfError::Empty => write!(f, "The file is empty."),
            DxfError::Malformed { line, reason } => {
                write!(
                    f,
                    "This doesn't look like a valid DXF file (line {line}: {reason})."
                )
            }
        }
    }
}

impl std::error::Error for DxfError {}

// ---------------------------------------------------------------------------------------------
// Low level: code/value pairs and raw entities

struct Pair<'a> {
    code: i32,
    value: &'a str,
}

fn read_pairs(text: &str) -> Result<Vec<Pair<'_>>, DxfError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.starts_with("AutoCAD Binary DXF") {
        return Err(DxfError::Binary);
    }
    if text.trim().is_empty() {
        return Err(DxfError::Empty);
    }
    let lines: Vec<&str> = text.lines().collect();
    let mut pairs = Vec::with_capacity(lines.len() / 2);
    let mut i = 0;
    while i < lines.len() {
        let code_line = lines[i].trim();
        if code_line.is_empty() && pairs.is_empty() {
            i += 1;
            continue;
        }
        let code: i32 = code_line.parse().map_err(|_| DxfError::Malformed {
            line: i + 1,
            reason: format!("expected a group code number, found \"{code_line}\""),
        })?;
        let value = lines
            .get(i + 1)
            .ok_or(DxfError::Malformed {
                line: i + 1,
                reason: "file ends in the middle of an entry".to_string(),
            })?
            .trim();
        pairs.push(Pair { code, value });
        if code == 0 && value == "EOF" {
            break;
        }
        i += 2;
    }
    Ok(pairs)
}

#[derive(Debug, Default)]
struct RawEntity<'a> {
    kind: &'a str,
    pairs: Vec<(i32, &'a str)>,
    /// POLYLINE vertices.
    children: Vec<RawEntity<'a>>,
}

impl<'a> RawEntity<'a> {
    fn str(&self, code: i32) -> Option<&'a str> {
        self.pairs.iter().find(|p| p.0 == code).map(|p| p.1)
    }
    /// First value with this code as a number. `Err` if present but not a number.
    fn num(&self, code: i32) -> Result<Option<f64>, ()> {
        match self.str(code) {
            None => Ok(None),
            Some(v) => v.parse::<f64>().map(Some).map_err(|_| ()),
        }
    }
    fn req(&self, code: i32) -> Result<f64, ()> {
        self.num(code)?.ok_or(())
    }
    fn opt(&self, code: i32, default: f64) -> Result<f64, ()> {
        Ok(self.num(code)?.unwrap_or(default))
    }
    fn int(&self, code: i32) -> i64 {
        self.str(code).and_then(|v| v.parse().ok()).unwrap_or(0)
    }
    fn all(&self, code: i32) -> Result<Vec<f64>, ()> {
        self.pairs
            .iter()
            .filter(|p| p.0 == code)
            .map(|p| p.1.parse::<f64>().map_err(|_| ()))
            .collect()
    }
    /// Sequential (x, y) points where `x_code` starts each point and `y_code` fills it in.
    fn points(&self, x_code: i32, y_code: i32) -> Result<Vec<Point>, ()> {
        let mut pts: Vec<Point> = Vec::new();
        for &(code, v) in &self.pairs {
            if code == x_code {
                pts.push(Point::new(v.parse().map_err(|_| ())?, 0.0));
            } else if code == y_code {
                pts.last_mut().ok_or(())?.y = v.parse().map_err(|_| ())?;
            }
        }
        Ok(pts)
    }
}

/// Reads entities starting at `pairs[*i]` until ENDSEC / ENDBLK (not consumed) or the end.
fn read_entities<'a>(pairs: &[Pair<'a>], i: &mut usize) -> Vec<RawEntity<'a>> {
    let mut out = Vec::new();
    while *i < pairs.len() {
        let p = &pairs[*i];
        if p.code == 0 && matches!(p.value, "ENDSEC" | "ENDBLK" | "EOF") {
            break;
        }
        let entity = read_one(pairs, i);
        match entity.kind {
            "POLYLINE" => {
                let mut poly = entity;
                while *i < pairs.len() && pairs[*i].code == 0 && pairs[*i].value == "VERTEX" {
                    poly.children.push(read_one(pairs, i));
                }
                if *i < pairs.len() && pairs[*i].code == 0 && pairs[*i].value == "SEQEND" {
                    read_one(pairs, i);
                }
                out.push(poly);
            }
            "INSERT" => {
                let has_attribs = entity.int(66) == 1;
                out.push(entity);
                if has_attribs {
                    while *i < pairs.len() && pairs[*i].code == 0 && pairs[*i].value == "ATTRIB" {
                        read_one(pairs, i);
                    }
                    if *i < pairs.len() && pairs[*i].code == 0 && pairs[*i].value == "SEQEND" {
                        read_one(pairs, i);
                    }
                }
            }
            _ => out.push(entity),
        }
    }
    out
}

/// Reads one entity: its `0` pair and all following non-`0` pairs.
fn read_one<'a>(pairs: &[Pair<'a>], i: &mut usize) -> RawEntity<'a> {
    let mut e = RawEntity {
        kind: pairs[*i].value,
        ..Default::default()
    };
    *i += 1;
    while *i < pairs.len() && pairs[*i].code != 0 {
        e.pairs.push((pairs[*i].code, pairs[*i].value));
        *i += 1;
    }
    e
}

/// Skips to just past the next ENDSEC.
fn skip_section(pairs: &[Pair<'_>], i: &mut usize) {
    while *i < pairs.len() {
        let done = pairs[*i].code == 0 && pairs[*i].value == "ENDSEC";
        *i += 1;
        if done {
            break;
        }
    }
}

struct Block<'a> {
    base: Point,
    entities: Vec<RawEntity<'a>>,
}

// ---------------------------------------------------------------------------------------------
// Public entry point

pub fn read_dxf(text: &str) -> Result<DxfDrawing, DxfError> {
    let pairs = read_pairs(text)?;
    let mut units = DxfUnits::from_code(0);
    let mut blocks: HashMap<String, Block> = HashMap::new();
    let mut entities: Vec<RawEntity> = Vec::new();

    let mut i = 0;
    while i < pairs.len() {
        let p = &pairs[i];
        if p.code == 0 && p.value == "EOF" {
            break;
        }
        if !(p.code == 0 && p.value == "SECTION") {
            i += 1;
            continue;
        }
        let name = pairs.get(i + 1).filter(|n| n.code == 2).map(|n| n.value);
        i += 2;
        match name {
            Some("HEADER") => {
                while i < pairs.len() && !(pairs[i].code == 0 && pairs[i].value == "ENDSEC") {
                    if pairs[i].code == 9
                        && pairs[i].value == "$INSUNITS"
                        && let Some(code) = pairs.get(i + 1).and_then(|v| v.value.parse().ok())
                    {
                        units = DxfUnits::from_code(code);
                    }
                    i += 1;
                }
                i += 1;
            }
            Some("BLOCKS") => {
                while i < pairs.len() && !(pairs[i].code == 0 && pairs[i].value == "ENDSEC") {
                    if pairs[i].code == 0 && pairs[i].value == "BLOCK" {
                        let header = read_one(&pairs, &mut i);
                        let ents = read_entities(&pairs, &mut i);
                        // Consume ENDBLK and its data.
                        if i < pairs.len() && pairs[i].value == "ENDBLK" {
                            read_one(&pairs, &mut i);
                        }
                        let base = Point::new(
                            header.opt(10, 0.0).unwrap_or(0.0),
                            header.opt(20, 0.0).unwrap_or(0.0),
                        );
                        if let Some(name) = header.str(2) {
                            blocks.insert(
                                name.to_string(),
                                Block {
                                    base,
                                    entities: ents,
                                },
                            );
                        }
                    } else {
                        i += 1;
                    }
                }
                i += 1;
            }
            Some("ENTITIES") => {
                entities = read_entities(&pairs, &mut i);
                skip_section(&pairs, &mut i);
            }
            _ => skip_section(&pairs, &mut i),
        }
    }

    let mut conv = Converter {
        blocks: &blocks,
        shapes: Vec::new(),
        points: Vec::new(),
        skipped: BTreeMap::new(),
    };
    for e in &entities {
        conv.convert(e, &Transform::IDENTITY, None, 0);
    }

    // POINT entities become one point list per layer (what the drill operation uses).
    let mut shapes = conv.shapes;
    for (layer, points) in conv.points {
        shapes.push((layer, Shape::Points { points }));
    }
    Ok(DxfDrawing {
        units,
        shapes,
        skipped: conv.skipped,
    })
}

// ---------------------------------------------------------------------------------------------
// Entity conversion

const MAX_BLOCK_DEPTH: usize = 16;

struct Converter<'b, 'a> {
    blocks: &'b HashMap<String, Block<'a>>,
    shapes: Vec<(String, Shape)>,
    /// Points grouped by layer, in first-seen layer order.
    points: Vec<(String, Vec<Point>)>,
    skipped: BTreeMap<String, usize>,
}

/// How an entity's own coordinate system (OCS) maps to the drawing.
enum Ocs {
    Normal,
    /// Extrusion direction (0, 0, -1): the entity is mirrored in X.
    Flipped,
    /// Tilted out of the XY plane: not 2D.
    Tilted,
}

fn ocs(e: &RawEntity) -> Ocs {
    let nx = e.opt(210, 0.0).unwrap_or(0.0);
    let ny = e.opt(220, 0.0).unwrap_or(0.0);
    let nz = e.opt(230, 1.0).unwrap_or(1.0);
    if nx.abs() > 1e-6 || ny.abs() > 1e-6 {
        Ocs::Tilted
    } else if nz < 0.0 {
        Ocs::Flipped
    } else {
        Ocs::Normal
    }
}

impl<'b, 'a> Converter<'b, 'a> {
    fn skip(&mut self, what: impl Into<String>) {
        *self.skipped.entry(what.into()).or_insert(0) += 1;
    }

    fn add(&mut self, layer: &str, shape: Shape, t: &Transform, kind: &str) {
        match shape.transformed(t) {
            Some(s) => self.shapes.push((layer.to_string(), s)),
            None => self.skip(format!("{kind} (unevenly scaled block)")),
        }
    }

    fn convert(&mut self, e: &RawEntity, t: &Transform, parent_layer: Option<&str>, depth: usize) {
        // Paper-space entities (layout sheets) are not part of the model.
        if e.int(67) == 1 {
            self.skip(format!("{} (paper space)", e.kind));
            return;
        }
        let own_layer = e.str(8).unwrap_or("0");
        // Block contents on layer "0" take the layer of the INSERT that places them.
        let layer = match parent_layer {
            Some(p) if own_layer == "0" => p.to_string(),
            _ => own_layer.to_string(),
        };

        // Entities whose coordinates are in their own coordinate system get the OCS correction.
        let uses_ocs = matches!(
            e.kind,
            "ARC" | "CIRCLE" | "LWPOLYLINE" | "POLYLINE" | "INSERT"
        );
        let local = if uses_ocs {
            match ocs(e) {
                Ocs::Normal => *t,
                Ocs::Flipped => Transform::mirror_x().then(*t),
                Ocs::Tilted => {
                    self.skip(format!("{} (not flat in XY)", e.kind));
                    return;
                }
            }
        } else {
            *t
        };

        let result: Result<(), ()> = (|| {
            match e.kind {
                "LINE" => {
                    let shape = Shape::Line {
                        start: Point::new(e.req(10)?, e.req(20)?),
                        end: Point::new(e.req(11)?, e.req(21)?),
                    };
                    self.add(&layer, shape, &local, e.kind);
                }
                "CIRCLE" => {
                    let shape = Shape::Circle {
                        center: Point::new(e.req(10)?, e.req(20)?),
                        radius: e.req(40)?,
                    };
                    self.add(&layer, shape, &local, e.kind);
                }
                "ARC" => {
                    let start = e.req(50)?;
                    let mut end = e.req(51)?;
                    if end <= start {
                        end += 360.0;
                    }
                    let shape = Shape::Arc {
                        center: Point::new(e.req(10)?, e.req(20)?),
                        radius: e.req(40)?,
                        start_angle: start,
                        end_angle: end,
                    };
                    self.add(&layer, shape, &local, e.kind);
                }
                "LWPOLYLINE" => {
                    let mut vertices: Vec<Vertex> = Vec::new();
                    for &(code, v) in &e.pairs {
                        match code {
                            10 => vertices.push(Vertex::line(v.parse().map_err(|_| ())?, 0.0)),
                            20 => vertices.last_mut().ok_or(())?.y = v.parse().map_err(|_| ())?,
                            42 => {
                                vertices.last_mut().ok_or(())?.bulge = v.parse().map_err(|_| ())?
                            }
                            _ => {}
                        }
                    }
                    self.add_polyline(&layer, vertices, e.int(70) & 1 == 1, &local, e.kind);
                }
                "POLYLINE" => {
                    let flags = e.int(70);
                    if flags & (8 | 16 | 64) != 0 {
                        self.skip("POLYLINE (3D or mesh)");
                        return Ok(());
                    }
                    let mut vertices = Vec::new();
                    for v in &e.children {
                        if v.int(70) & 16 != 0 {
                            continue; // spline frame control point, not on the curve
                        }
                        vertices.push(Vertex {
                            x: v.req(10)?,
                            y: v.req(20)?,
                            bulge: v.opt(42, 0.0)?,
                        });
                    }
                    self.add_polyline(&layer, vertices, flags & 1 == 1, &local, e.kind);
                }
                "POINT" => {
                    let p = local.apply(Point::new(e.req(10)?, e.req(20)?));
                    match self.points.iter_mut().find(|(l, _)| *l == layer) {
                        Some((_, pts)) => pts.push(p),
                        None => self.points.push((layer.clone(), vec![p])),
                    }
                }
                "SPLINE" => {
                    let control_points = e.points(10, 20)?;
                    if control_points.is_empty() {
                        self.skip("SPLINE (fit points only)");
                        return Ok(());
                    }
                    let degree = e.int(71).max(1) as usize;
                    let knots = e.all(40)?;
                    if knots.len() != control_points.len() + degree + 1 {
                        self.skip("SPLINE (inconsistent data)");
                        return Ok(());
                    }
                    let mut weights = e.all(41)?;
                    if weights.len() != control_points.len() || weights.iter().all(|w| *w == 1.0) {
                        weights.clear();
                    }
                    let shape = Shape::Spline(Spline {
                        degree,
                        control_points,
                        knots,
                        weights,
                        closed: e.int(70) & 1 == 1,
                    });
                    self.add(&layer, shape, &local, e.kind);
                }
                "ELLIPSE" => {
                    let mut start = e.opt(41, 0.0)?;
                    let mut end = e.opt(42, TAU)?;
                    // An ELLIPSE with a downward extrusion runs clockwise: parameter t becomes -t.
                    if e.opt(230, 1.0)? < 0.0 {
                        (start, end) = (-end, -start);
                    }
                    if end <= start {
                        end += TAU;
                    }
                    if start < 0.0 {
                        start += TAU;
                        end += TAU;
                    }
                    let shape = Shape::Ellipse {
                        center: Point::new(e.req(10)?, e.req(20)?),
                        major_axis: Point::new(e.req(11)?, e.req(21)?),
                        ratio: e.req(40)?,
                        start_param: start,
                        end_param: end,
                    };
                    self.add(&layer, shape, &local, e.kind);
                }
                "INSERT" => self.insert(e, &local, &layer, depth)?,
                other => self.skip(other),
            }
            Ok(())
        })();
        if result.is_err() {
            self.skip(format!("{} (unreadable)", e.kind));
        }
    }

    fn add_polyline(
        &mut self,
        layer: &str,
        vertices: Vec<Vertex>,
        closed: bool,
        t: &Transform,
        kind: &str,
    ) {
        if vertices.len() < 2 {
            self.skip(format!("{kind} (fewer than 2 points)"));
            return;
        }
        self.add(
            layer,
            Shape::Polyline(Polyline { vertices, closed }),
            t,
            kind,
        );
    }

    fn insert(
        &mut self,
        e: &RawEntity,
        t: &Transform,
        layer: &str,
        depth: usize,
    ) -> Result<(), ()> {
        let name = e.str(2).ok_or(())?;
        let Some(block) = self.blocks.get(name) else {
            self.skip("INSERT (missing block)");
            return Ok(());
        };
        if depth >= MAX_BLOCK_DEPTH {
            self.skip("INSERT (nested too deep)");
            return Ok(());
        }
        let pos = Point::new(e.opt(10, 0.0)?, e.opt(20, 0.0)?);
        let (sx, sy) = (e.opt(41, 1.0)?, e.opt(42, 1.0)?);
        let rotation = e.opt(50, 0.0)?;
        let cols = e.int(70).max(1);
        let rows = e.int(71).max(1);
        let (col_gap, row_gap) = (e.opt(44, 0.0)?, e.opt(45, 0.0)?);
        let blocks = self.blocks;
        let base = block.base;
        for r in 0..rows {
            for c in 0..cols {
                let placement = Transform::translate(-base.x, -base.y)
                    .then(Transform::scale(sx, sy))
                    .then(Transform::translate(c as f64 * col_gap, r as f64 * row_gap))
                    .then(Transform::rotate_degrees(rotation))
                    .then(Transform::translate(pos.x, pos.y))
                    .then(*t);
                for child in &blocks[name].entities {
                    self.convert(child, &placement, Some(layer), depth + 1);
                }
            }
        }
        Ok(())
    }
}
