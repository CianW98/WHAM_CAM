//! Offset adapter tests, and canaries for the cavalier_contours workarounds (CLAUDE.md:
//! libraries that shape toolpaths). A canary passes while the library problem still exists;
//! if one fails after a library update, the workaround it guards may no longer be needed.

use cavalier_contours::polyline::{PlineSource, PlineSourceMut, Polyline as CcPolyline};
use cavalier_contours::shape_algorithms::{Shape as CcShape, ShapeOffsetOptions};
use wham_engine::display::polyline_signed_area;
use wham_engine::geometry::{Point, Polyline, Vertex};
use wham_engine::offset::{LoopSide, contains, offset_area, offset_closed, outlines_cross};

fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Polyline {
    Polyline {
        vertices: vec![
            Vertex::line(x0, y0),
            Vertex::line(x1, y0),
            Vertex::line(x1, y1),
            Vertex::line(x0, y1),
        ],
        closed: true,
    }
}

/// A 270° arc of radius 20 about the origin (symmetric about +X), closed by its chord: one
/// segment with a bulge over 1.
fn big_arc() -> Polyline {
    let deg: f64 = 270.0;
    let (a0, a1) = ((-deg / 2.0).to_radians(), (deg / 2.0).to_radians());
    Polyline {
        vertices: vec![
            Vertex {
                x: 20.0 * a0.cos(),
                y: 20.0 * a0.sin(),
                bulge: (deg.to_radians() / 4.0).tan(),
            },
            Vertex::line(20.0 * a1.cos(), 20.0 * a1.sin()),
        ],
        closed: true,
    }
}

fn total_area(loops: &[wham_engine::offset::OffsetLoop]) -> f64 {
    loops
        .iter()
        .map(|l| polyline_signed_area(&l.polyline))
        .sum()
}

/// Canary for cavalier_contours issue #15 (bulge over 1). With an island and an 8 mm offset,
/// handing the library the 270° arc as one segment gives a wildly wrong result.
#[test]
fn canary_issue_15_library_mishandles_arcs_over_180() {
    let arc = big_arc();
    let mut whole = CcPolyline::new_closed();
    for v in &arc.vertices {
        whole.add(v.x, v.y, v.bulge);
    }
    let mut island = CcPolyline::new_closed();
    for (x, y) in [(-3.0, -3.0), (-3.0, 3.0), (3.0, 3.0), (3.0, -3.0)] {
        island.add(x, y, 0.0); // clockwise
    }
    let result =
        CcShape::from_plines([whole, island]).parallel_offset(8.0, &ShapeOffsetOptions::new());
    let area: f64 = result
        .ccw_plines
        .iter()
        .chain(&result.cw_plines)
        .map(|p| p.polyline.area())
        .sum();
    // Our adapter (which splits the arc) gives about +20.7; the library alone gives about -400.
    assert!(
        area < 0.0,
        "cavalier_contours now handles this case (area {area}): the arc-splitting workaround in offset.rs may no longer be needed"
    );
}

#[test]
fn adapter_offsets_big_arcs_correctly() {
    let island = square(-3.0, -3.0, 3.0, 3.0);
    let loops = offset_area(&big_arc(), &[island], 8.0).unwrap();
    let area = total_area(&loops);
    assert!((area - 20.716).abs() < 0.01, "area {area}");
}

#[test]
fn offset_area_shrinks_boundary_and_grows_islands() {
    let loops = offset_area(
        &square(0.0, 0.0, 100.0, 80.0),
        &[square(40.0, 30.0, 60.0, 50.0)],
        3.0,
    )
    .unwrap();
    let outer: Vec<_> = loops.iter().filter(|l| l.side == LoopSide::Outer).collect();
    let holes: Vec<_> = loops
        .iter()
        .filter(|l| l.side == LoopSide::Island)
        .collect();
    assert_eq!((outer.len(), holes.len()), (1, 1));
    assert!((polyline_signed_area(&outer[0].polyline) - 94.0 * 74.0).abs() < 1e-6);
    // The island grows by 3 mm with rounded corners: 20×20 + 4·(20·3) + π·3².
    let grown = 400.0 + 240.0 + std::f64::consts::PI * 9.0;
    assert!((polyline_signed_area(&holes[0].polyline).abs() - grown).abs() < 1e-6);
    // Gone once the offset passes the middle.
    assert!(
        offset_area(&square(0.0, 0.0, 10.0, 10.0), &[], 5.5)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn containment_and_crossing() {
    let big = square(0.0, 0.0, 10.0, 10.0);
    assert!(contains(&big, Point::new(5.0, 5.0)).unwrap());
    assert!(!contains(&big, Point::new(15.0, 5.0)).unwrap());
    // Direction of the outline doesn't matter.
    assert!(contains(&big.reversed(), Point::new(5.0, 5.0)).unwrap());
    assert!(outlines_cross(&big, &square(5.0, 5.0, 15.0, 15.0)).unwrap());
    assert!(!outlines_cross(&big, &square(2.0, 2.0, 4.0, 4.0)).unwrap());
}

#[test]
fn circle_as_two_half_arcs_offsets_to_a_smaller_circle() {
    let circle = Polyline {
        vertices: vec![
            Vertex {
                x: 10.0,
                y: 0.0,
                bulge: 1.0,
            },
            Vertex {
                x: -10.0,
                y: 0.0,
                bulge: 1.0,
            },
        ],
        closed: true,
    };
    let loops = offset_area(&circle, &[], 3.0).unwrap();
    assert_eq!(loops.len(), 1);
    let a = polyline_signed_area(&loops[0].polyline);
    assert!((a - std::f64::consts::PI * 49.0).abs() < 1e-6, "{a}");
}

#[test]
fn offset_closed_grows_with_corner_arcs_and_shrinks_with_sharp_corners() {
    let sq = square(0.0, 0.0, 20.0, 20.0);
    let grown = offset_closed(&sq, 3.0).unwrap();
    assert_eq!(grown.len(), 1);
    assert_eq!(grown[0].side, LoopSide::Outer);
    let g = &grown[0].polyline;
    assert_eq!(g.vertices.iter().filter(|v| v.bulge != 0.0).count(), 4);
    // 26 x 26 square with its corners rounded off by radius 3: 26² − (4 − π)·9.
    let want = 26.0 * 26.0 - (4.0 - std::f64::consts::PI) * 9.0;
    assert!((polyline_signed_area(g) - want).abs() < 1e-6);

    let shrunk = offset_closed(&sq, -3.0).unwrap();
    assert_eq!(shrunk.len(), 1);
    let s = &shrunk[0].polyline;
    assert!(s.vertices.iter().all(|v| v.bulge == 0.0));
    assert!((polyline_signed_area(s) - 14.0 * 14.0).abs() < 1e-6);

    assert!(offset_closed(&sq, -10.5).unwrap().is_empty());
}
