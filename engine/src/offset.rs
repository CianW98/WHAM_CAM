//! Offsetting, through the cavalier_contours library. This is the ONLY file that uses the library
//! (CLAUDE.md: libraries that shape toolpaths). The rest of the engine works in our own types.
//!
//! Workarounds for library problems, each with a canary test in `engine/tests/offset.rs`:
//! - Arcs over 180° (bulge beyond ±1) are split in two before being handed over: the library
//!   only supports up to half-circle arcs and can give badly wrong offsets otherwise
//!   (cavalier_contours issue #15).

use std::panic::{AssertUnwindSafe, catch_unwind};

use cavalier_contours::core::math::Vector2;
use cavalier_contours::polyline::{
    PlineOrientation, PlineSource, PlineSourceMut, Polyline as CcPolyline,
};
use cavalier_contours::shape_algorithms::{Shape as CcShape, ShapeOffsetOptions};

use crate::geometry::{Point, Polyline, Vertex, bulge_arc, bulge_from_sweep};

/// Which side of the area a loop bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopSide {
    /// Bounds the area from outside (an outer wall). Comes back anticlockwise.
    Outer,
    /// Bounds a hole in the area (the wall around an island). Comes back clockwise.
    Island,
}

/// One closed loop of an offset result.
#[derive(Debug, Clone, PartialEq)]
pub struct OffsetLoop {
    pub polyline: Polyline,
    pub side: LoopSide,
}

/// Runs library code, turning a panic into an error message so it can never crash WHAM_CAM.
/// (Native builds only: a WebAssembly build stops on a panic instead, so there the tests and the
/// toolpath safety checker are the protection.)
fn guarded<T>(what: &str, f: impl FnOnce() -> T) -> Result<T, String> {
    catch_unwind(AssertUnwindSafe(f)).map_err(|_| {
        format!("The offsetting library failed while {what}. Try simplifying the shapes.")
    })
}

/// Our closed polyline as a library polyline, turned to the wanted direction. Arcs over 180°
/// are split in two (issue #15). A last point repeating the first is dropped.
fn to_cc(pl: &Polyline, ccw: bool) -> CcPolyline<f64> {
    let mut v = pl.vertices.clone();
    if v.len() > 1 && v[0].point().distance_to(v[v.len() - 1].point()) < 1e-9 {
        v.pop(); // its segment (back to the first point) has no length
    }
    let n = v.len();
    let mut out = CcPolyline::new_closed();
    for i in 0..n {
        let (a, b) = (v[i], v[(i + 1) % n]);
        match bulge_arc(a.point(), b.point(), a.bulge) {
            Some(arc) if a.bulge.abs() > 1.0 => {
                let half = bulge_from_sweep(arc.sweep / 2.0);
                let m = arc.mid_point();
                out.add(a.x, a.y, half);
                out.add(m.x, m.y, half);
            }
            _ => out.add(a.x, a.y, a.bulge),
        }
    }
    let is_ccw = out.orientation() == PlineOrientation::CounterClockwise;
    if is_ccw != ccw {
        out.invert_direction_mut();
    }
    out
}

fn from_cc(pl: &impl PlineSource<Num = f64>) -> Polyline {
    Polyline {
        vertices: pl
            .iter_vertexes()
            .map(|v| Vertex {
                x: v.x,
                y: v.y,
                bulge: v.bulge,
            })
            .collect(),
        closed: true,
    }
}

/// Offsets an area, a closed `boundary` with closed `islands` (holes) inside it, inward by
/// `distance` (mm, more than 0): the boundary shrinks and the islands grow, with rounded
/// (tool-radius) corners where they grow. Returns the resulting loops; empty if nothing is left.
pub fn offset_area(
    boundary: &Polyline,
    islands: &[Polyline],
    distance: f64,
) -> Result<Vec<OffsetLoop>, String> {
    let mut plines = vec![to_cc(boundary, true)];
    plines.extend(islands.iter().map(|i| to_cc(i, false)));
    let result = guarded("offsetting a pocket", || {
        CcShape::from_plines(plines).parallel_offset(distance, &ShapeOffsetOptions::new())
    })?;
    let outer = result.ccw_plines.iter().map(|p| OffsetLoop {
        polyline: from_cc(&p.polyline),
        side: LoopSide::Outer,
    });
    let holes = result.cw_plines.iter().map(|p| OffsetLoop {
        polyline: from_cc(&p.polyline),
        side: LoopSide::Island,
    });
    Ok(outer.chain(holes).collect())
}

/// Offsets one closed shape by `distance` (mm): more than 0 grows it (outside), less than 0
/// shrinks it (inside). Growing rounds the corners with arcs of `distance`. Returns the resulting
/// loops: `Outer` loops (anticlockwise) and, where growing closes off a hollow, `Island` loops
/// (clockwise) inside it. Empty if nothing is left.
pub fn offset_closed(pl: &Polyline, distance: f64) -> Result<Vec<OffsetLoop>, String> {
    let shape = vec![to_cc(pl, true)];
    let result = guarded("offsetting a profile", || {
        // The library offsets anticlockwise shapes inward for a positive distance.
        CcShape::from_plines(shape).parallel_offset(-distance, &ShapeOffsetOptions::new())
    })?;
    let outer = result.ccw_plines.iter().map(|p| OffsetLoop {
        polyline: from_cc(&p.polyline),
        side: LoopSide::Outer,
    });
    let holes = result.cw_plines.iter().map(|p| OffsetLoop {
        polyline: from_cc(&p.polyline),
        side: LoopSide::Island,
    });
    Ok(outer.chain(holes).collect())
}

/// True if `p` is inside the closed `outline` (on the line counts as outside or inside, so
/// test with a point that isn't on another outline).
pub fn contains(outline: &Polyline, p: Point) -> Result<bool, String> {
    let pl = to_cc(outline, true);
    guarded("testing which shapes are inside others", || {
        pl.winding_number(Vector2::new(p.x, p.y)) != 0
    })
}

/// True if two closed outlines cross or touch.
pub fn outlines_cross(a: &Polyline, b: &Polyline) -> Result<bool, String> {
    let (a, b) = (to_cc(a, true), to_cc(b, true));
    guarded("checking whether shapes cross", || {
        let found = a.find_intersects(&b);
        !found.basic_intersects.is_empty() || !found.overlapping_intersects.is_empty()
    })
}

/// True if the straight line from `a` to `b` crosses or touches the closed `outline`.
pub fn segment_crosses(a: Point, b: Point, outline: &Polyline) -> Result<bool, String> {
    let mut seg = CcPolyline::new();
    seg.add(a.x, a.y, 0.0);
    seg.add(b.x, b.y, 0.0);
    let pl = to_cc(outline, true);
    guarded("checking a link move", || {
        let found = seg.find_intersects(&pl);
        !found.basic_intersects.is_empty() || !found.overlapping_intersects.is_empty()
    })
}
