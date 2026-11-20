//! Pocket toolpaths (spec 6.3): rings offset from the boundary and around islands, cut from the
//! inside out, at each depth level.

use crate::geometry::{Point, Polyline};
use crate::offset::{self, LoopSide, OffsetLoop};
use crate::operation::{
    CommonSettings, CutOrdering, MillingDirection, PocketSettings, StepoverFeed,
};
pub use crate::path::closed_outline;
use crate::path::{Builder, nearest_on_loop, start_nearest};
use crate::toolpath::{Limits, Move, Point3, Toolpath, depth_levels};

/// One area to pocket: a boundary with the islands directly inside it.
#[derive(Debug, Clone)]
struct Region {
    /// The boundary shape's id (for messages).
    id: u32,
    boundary: Polyline,
    islands: Vec<Polyline>,
}

/// Sorts outlines into areas by nesting: inside a boundary = island; inside an island = a new
/// boundary. Outlines that cross are refused.
fn regions(outlines: &[(u32, Polyline)]) -> Result<Vec<Region>, String> {
    for (i, (ia, a)) in outlines.iter().enumerate() {
        for (ib, b) in &outlines[i + 1..] {
            if offset::outlines_cross(a, b)? {
                return Err(format!(
                    "Shapes {ia} and {ib} cross or touch. Pocket shapes must not cross (use Break at intersections and Join to make separate outlines)."
                ));
            }
        }
    }
    // For each outline, the outlines it lies inside.
    let mut inside: Vec<Vec<usize>> = Vec::new();
    for (i, (_, a)) in outlines.iter().enumerate() {
        let p = a.vertices[0].point();
        let mut list = Vec::new();
        for (j, (_, b)) in outlines.iter().enumerate() {
            if i != j && offset::contains(b, p)? {
                list.push(j);
            }
        }
        inside.push(list);
    }
    let depth = |i: usize| inside[i].len();
    let mut regions: Vec<(usize, Region)> = Vec::new();
    for (i, (id, outline)) in outlines.iter().enumerate() {
        if depth(i) % 2 == 0 {
            regions.push((
                i,
                Region {
                    id: *id,
                    boundary: outline.clone(),
                    islands: Vec::new(),
                },
            ));
        }
    }
    for (i, (_, outline)) in outlines.iter().enumerate() {
        if depth(i) % 2 == 1 {
            // Its parent is the containing outline one level up.
            let parent = inside[i]
                .iter()
                .copied()
                .find(|&j| depth(j) + 1 == depth(i))
                .expect("an odd-depth outline is inside an even-depth one");
            let r = regions
                .iter_mut()
                .find(|(j, _)| *j == parent)
                .expect("every even-depth outline is a region");
            r.1.islands.push(outline.clone());
        }
    }
    Ok(regions.into_iter().map(|(_, r)| r).collect())
}

/// The rings of a region, outermost (the wall cut) first. Each entry is one offset distance and
/// may hold several loops (an area split in two, or the rings around islands).
fn rings(
    region: &Region,
    first: f64,
    step: f64,
    radius: f64,
) -> Result<Vec<Vec<OffsetLoop>>, String> {
    let at = |d: f64| offset::offset_area(&region.boundary, &region.islands, d);
    let mut sets = Vec::new();
    loop {
        // first + k·step (not a running sum, so distances don't drift).
        let set = at(first + sets.len() as f64 * step)?;
        if set.is_empty() {
            break;
        }
        sets.push(set);
        if sets.len() > 100_000 {
            return Err("Too many pocket rings; check the stepover.".into());
        }
    }
    if sets.is_empty() {
        return Err(format!(
            "The tool is too big to fit in the pocket (shape {}).",
            region.id
        ));
    }
    // With a stepover over half the tool, a ring can vanish leaving a spot in its middle more
    // than a tool radius from any ring. Add a ring there.
    if step > radius {
        let mut k = 0;
        while k < sets.len() {
            let probe = at(first + k as f64 * step + radius)?;
            let next_outer: Vec<Point> = sets
                .get(k + 1)
                .map(|s| {
                    s.iter()
                        .filter(|l| l.side == LoopSide::Outer)
                        .map(|l| l.polyline.vertices[0].point())
                        .collect()
                })
                .unwrap_or_default();
            let mut extra = Vec::new();
            for l in probe.iter().filter(|l| l.side == LoopSide::Outer) {
                let mut covered = false;
                for p in &next_outer {
                    if offset::contains(&l.polyline, *p)? {
                        covered = true;
                        break;
                    }
                }
                if !covered {
                    extra.push(l.clone());
                    for isl in probe.iter().filter(|i| i.side == LoopSide::Island) {
                        if offset::contains(&l.polyline, isl.polyline.vertices[0].point())? {
                            extra.push(isl.clone());
                        }
                    }
                }
            }
            if !extra.is_empty() {
                if k + 1 < sets.len() {
                    sets[k + 1].extend(extra);
                } else {
                    sets.push(extra);
                }
            }
            k += 1;
        }
    }
    Ok(sets)
}

/// A pocket's toolpath. `outlines` are the selected shapes as closed outlines, with their ids
/// (for messages). Errors are plain-English messages.
pub fn pocket_toolpath(
    operation_id: u32,
    outlines: &[(u32, Polyline)],
    c: &CommonSettings,
    p: &PocketSettings,
) -> Result<Toolpath, String> {
    let target = c.target_depth.ok_or("Set the target depth.")?;
    if outlines.is_empty() {
        return Err(
            "The pocket has no shapes. Select closed shapes and use Use selected shapes.".into(),
        );
    }
    let radius = c.tool_diameter / 2.0;
    let first = radius + c.roughing_clearance;
    let step = p.stepover * c.tool_diameter;

    let regions = regions(outlines)?;
    let mut region_rings = Vec::new();
    for r in &regions {
        region_rings.push(rings(r, first, step, radius)?);
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
    let link_feed = match p.stepover_feed {
        StepoverFeed::Plunge => c.plunge_feedrate,
        StepoverFeed::Cut => c.cut_feedrate,
    };

    let cut = |b: &mut Builder, ri: usize, li: usize| -> Result<(), String> {
        let z = levels[li];
        let above = if li == 0 {
            c.stock_surface
        } else {
            levels[li - 1]
        };
        let sets = &region_rings[ri];
        let islands = &regions[ri].islands;
        let mut first_loop = true;
        // Inside out: the last (innermost) set first.
        for k in (0..sets.len()).rev() {
            let mut left: Vec<&OffsetLoop> = sets[k].iter().collect();
            while !left.is_empty() {
                let here = b.xy();
                let (pick, _) = left
                    .iter()
                    .enumerate()
                    .map(|(i, l)| (i, nearest_on_loop(&l.polyline, here).3))
                    .fold((0, f64::INFINITY), |best, (i, d)| {
                        if d < best.1 - 1e-9 { (i, d) } else { best }
                    });
                let l = left.remove(pick);
                // Conventional: inside walls clockwise, island walls anticlockwise. The library
                // gives outer loops anticlockwise and island loops clockwise, so reverse both.
                let oriented = match p.milling_direction {
                    MillingDirection::Conventional => l.polyline.reversed(),
                    MillingDirection::Climb => l.polyline.clone(),
                };
                let start_loop = start_nearest(&oriented, here);
                let s = start_loop.vertices[0].point();
                if first_loop {
                    b.approach(s, above, z);
                    first_loop = false;
                } else {
                    // A short link inside the cleared band is fed; anything else retracts.
                    let short = here.distance_to(s) <= 2.0 * step + 1e-9;
                    let mut clear = short;
                    if short {
                        let neighbours = sets[k]
                            .iter()
                            .chain(sets.get(k + 1).into_iter().flatten())
                            .map(|o| &o.polyline)
                            .filter(|o| *o != &l.polyline)
                            .chain(islands.iter());
                        for o in neighbours {
                            if !touches_end(o, here) && offset::segment_crosses(here, s, o)? {
                                clear = false;
                                break;
                            }
                        }
                    }
                    if clear {
                        b.push(Move::Feed {
                            to: Point3::new(s.x, s.y, z),
                            feed: link_feed,
                        });
                    } else {
                        b.approach(s, above, z);
                    }
                }
                b.cut_loop(&start_loop, z);
            }
        }
        Ok(())
    };

    match p.cut_ordering {
        CutOrdering::DepthFirst => {
            for ri in 0..regions.len() {
                for li in 0..levels.len() {
                    cut(&mut b, ri, li)?;
                }
            }
        }
        CutOrdering::LevelFirst => {
            for li in 0..levels.len() {
                for ri in 0..regions.len() {
                    cut(&mut b, ri, li)?;
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

/// True if `p` lies on the loop `o` (the loop just cut ends there, so a link starting on it
/// isn't a crossing).
fn touches_end(o: &Polyline, p: Point) -> bool {
    nearest_on_loop(o, p).3 < 1e-6
}
