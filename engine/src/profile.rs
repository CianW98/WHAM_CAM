//! Profile toolpaths (spec 6.2): the tool follows each shape Inside, Outside or On line, at each
//! depth level, with a straight plunge at the shape's start point.

use crate::display::polyline_signed_area;
use crate::geometry::{Point, Polyline};
use crate::offset::{self, LoopSide};
use crate::operation::{
    CommonSettings, CutOrdering, MillingDirection, ProfileSettings, ProfileSide,
};
use crate::path::{Builder, nearest_on_loop, start_nearest};
use crate::toolpath::{Limits, Toolpath, depth_levels};

/// One continuous cut: round a closed loop (starting and ending at its first vertex) or along an
/// open path (first vertex to last).
#[derive(Debug, Clone)]
enum Cut {
    Loop(Polyline),
    Open(Polyline),
}

impl Cut {
    fn start(&self) -> Point {
        match self {
            Cut::Loop(pl) | Cut::Open(pl) => pl.vertices[0].point(),
        }
    }

    /// Where the tool is when the cut is done.
    fn end(&self) -> Point {
        match self {
            Cut::Loop(pl) => pl.vertices[0].point(),
            Cut::Open(pl) => pl.vertices[pl.vertices.len() - 1].point(),
        }
    }
}

/// `pl` turned to run anticlockwise (`ccw`) or clockwise.
fn oriented(pl: &Polyline, ccw: bool) -> Polyline {
    if (polyline_signed_area(pl) > 0.0) == ccw {
        pl.clone()
    } else {
        pl.reversed()
    }
}

/// The cuts for one shape, in cutting order.
fn shape_cuts(
    id: u32,
    pl: &Polyline,
    p: &ProfileSettings,
    offset_by: f64,
) -> Result<Vec<Cut>, String> {
    if !pl.closed {
        if p.side != ProfileSide::OnLine {
            return Err(format!(
                "Shape {id} is open: open shapes can only be cut On line."
            ));
        }
        // Open shapes follow their drawn direction.
        return Ok(vec![Cut::Open(pl.clone())]);
    }
    let first = pl.vertices[0].point();
    // Each loop with whether it is an inside wall (the material is outside the loop).
    let loops: Vec<(Polyline, bool)> = match p.side {
        ProfileSide::OnLine => vec![(pl.clone(), false)],
        ProfileSide::Outside => offset::offset_closed(pl, offset_by)?
            .into_iter()
            .map(|l| (l.polyline, l.side == LoopSide::Island))
            .collect(),
        ProfileSide::Inside => {
            let loops = offset::offset_closed(pl, -offset_by)?;
            if loops.is_empty() {
                return Err(format!("The tool is too big to cut inside shape {id}."));
            }
            loops.into_iter().map(|l| (l.polyline, true)).collect()
        }
    };
    // Conventional (spindle CW): outside walls anticlockwise, inside walls clockwise. Climb is
    // the reverse. Each loop starts at its point nearest the shape's first point.
    let mut cuts: Vec<Cut> = loops
        .iter()
        .map(|(l, inside_wall)| {
            let ccw = !inside_wall == (p.milling_direction == MillingDirection::Conventional);
            Cut::Loop(start_nearest(&oriented(l, ccw), first))
        })
        .collect();
    // Several loops (e.g. an inside offset split at a narrow waist): the one nearest the first
    // point, then nearest next.
    let mut ordered = Vec::with_capacity(cuts.len());
    let mut here = first;
    while !cuts.is_empty() {
        let i = nearest_index(&cuts, |c| match c {
            Cut::Loop(pl) => nearest_on_loop(pl, here).3,
            Cut::Open(_) => c.start().distance_to(here),
        });
        let c = cuts.remove(i);
        here = c.end();
        ordered.push(c);
    }
    Ok(ordered)
}

/// Index of the item with the smallest `dist` (the earlier one on a tie).
fn nearest_index<T>(items: &[T], dist: impl Fn(&T) -> f64) -> usize {
    let mut best = (0, f64::INFINITY);
    for (i, it) in items.iter().enumerate() {
        let d = dist(it);
        if d < best.1 - 1e-9 {
            best = (i, d);
        }
    }
    best.0
}

/// A profile's toolpath. `paths` are the selected shapes as closed or open polylines, with their
/// ids (for messages). Errors are plain-English messages.
pub fn profile_toolpath(
    operation_id: u32,
    paths: &[(u32, Polyline)],
    c: &CommonSettings,
    p: &ProfileSettings,
) -> Result<Toolpath, String> {
    let target = c.target_depth.ok_or("Set the target depth.")?;
    if paths.is_empty() {
        return Err("The profile has no shapes. Select shapes and use Use selected shapes.".into());
    }
    let offset_by = c.tool_diameter / 2.0 + c.roughing_clearance;
    let mut shapes = Vec::new();
    for (id, pl) in paths {
        shapes.push(shape_cuts(*id, pl, p, offset_by)?);
    }
    shapes.retain(|s| !s.is_empty());

    // The shape listed first, then always the nearest shape not yet cut.
    let mut order = Vec::with_capacity(shapes.len());
    let mut left: Vec<usize> = (0..shapes.len()).collect();
    let mut here: Option<Point> = None;
    while !left.is_empty() {
        let k = match here {
            None => 0,
            Some(h) => nearest_index(&left, |&i| shapes[i][0].start().distance_to(h)),
        };
        let i = left.remove(k);
        here = shapes[i].last().map(Cut::end);
        order.push(i);
    }

    let levels = depth_levels(
        c.stock_surface,
        target,
        c.depth_increment,
        c.final_depth_increment,
    );
    let mut b = Builder {
        c,
        moves: Vec::new(),
        at: None,
    };
    let cut = |b: &mut Builder, si: usize, li: usize| {
        let z = levels[li];
        let above = if li == 0 {
            c.stock_surface
        } else {
            levels[li - 1]
        };
        for cut in &shapes[si] {
            b.approach(cut.start(), above, z);
            match cut {
                Cut::Loop(pl) => b.cut_loop(pl, z),
                Cut::Open(pl) => b.cut_open(pl, z),
            }
        }
    };
    match p.cut_ordering {
        CutOrdering::DepthFirst => {
            for &si in &order {
                for li in 0..levels.len() {
                    cut(&mut b, si, li);
                }
            }
        }
        CutOrdering::LevelFirst => {
            for li in 0..levels.len() {
                for &si in &order {
                    cut(&mut b, si, li);
                }
            }
        }
    }
    b.retract();

    Ok(Toolpath {
        operation_id,
        moves: b.moves,
        limits: Limits {
            clearance_plane: c.clearance_plane,
            stock_surface: c.stock_surface,
            target_depth: target,
        },
        cycles: Vec::new(),
    })
}
