//! Known-good G-code (spec 8): the reference projects (Profile, Pocket, Drill, as in the reference
//! examples, and the Arcs test shape) written by the generic post-processor, compared exactly with
//! the checked files in `engine/tests/reference_gcode/` (see the README there).
//!
//! To write fresh files for checking, run:
//!   cargo test -p wham_engine --test reference_gcode -- --ignored
//! which writes them to `engine/tests/reference_gcode/pending/` (git-ignored). After the user has
//! checked them in a simulator and with an air cut, they are moved up into the folder itself.

use std::path::PathBuf;

use wham_engine::geometry::{Polyline, Vertex};
use wham_engine::operation::{MillingDirection, OperationKind, ProfileSide};
use wham_engine::*;

/// A fixed date, so the files don't change from day to day.
const DATE: &str = "reference";

fn folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/reference_gcode")
}

/// A project with one operation from `add`, using a `diameter` mm end mill as T1, with the
/// operation's settings changed by `edit`, generated.
fn job(
    shape: Shape,
    diameter: f64,
    add: fn(&mut Project, &[u32]) -> Result<u32, String>,
    edit: impl FnOnce(&mut Operation),
) -> Project {
    let mut p = Project::new();
    p.set_machining_tool(Some(PickedTool {
        number: 1,
        tool: Tool {
            diameter,
            ..Tool::new_default()
        }
        .named(),
    }))
    .unwrap();
    let id = p.add_shape("Default", shape).unwrap();
    let op = add(&mut p, &[id]).unwrap();
    let mut o = p.operation(op).unwrap().clone();
    edit(&mut o);
    p.set_operation(o).unwrap();
    let report = p.generate(&[]);
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    p
}

/// The reference Profile: the 8-point polyline with arcs, 6 mm tool, Outside, Conventional,
/// target −5 in one pass, clearance 10, feeds 1200 / 600, spindle 1000.
fn profile() -> Project {
    let v = |x: f64, y: f64, bulge: f64| Vertex { x, y, bulge };
    let shape = Shape::Polyline(Polyline {
        vertices: vec![
            v(-13.5991651223692, 29.0712686279469, 0.0),
            v(-10.0, 39.0, 0.0),
            v(64.0, 5.0, 0.0),
            v(31.0, -36.0, 0.0),
            v(-39.0, -41.0, 0.0),
            v(-30.9753411612932, -18.8630101001193, -0.04098469246601959),
            v(-34.747548783982, -17.0794021658196, -0.5773502691896257),
            v(-34.747548783982, 27.0794021658196, -0.218213101023905),
        ],
        closed: true,
    });
    job(shape, 6.0, Project::add_profile, |o| {
        let c = &mut o.common;
        c.target_depth = Some(-5.0);
        c.depth_increment = 5.0;
        c.clearance_plane = 10.0;
        c.cut_feedrate = 1200.0;
        c.plunge_feedrate = 600.0;
        c.spindle_speed = 1000.0;
    })
}

/// The reference Pocket: a 100 × 80 rectangle, 3 mm tool, stepover 0.4, target −10 in 5 mm
/// passes, clearance 3, feeds 800 / 300, spindle 1000.
fn pocket() -> Project {
    let shape = Shape::Polyline(Polyline {
        vertices: vec![
            Vertex::line(0.0, 0.0),
            Vertex::line(100.0, 0.0),
            Vertex::line(100.0, 80.0),
            Vertex::line(0.0, 80.0),
        ],
        closed: true,
    });
    job(shape, 3.0, Project::add_pocket, |o| {
        let c = &mut o.common;
        c.target_depth = Some(-10.0);
        c.depth_increment = 5.0;
        c.clearance_plane = 3.0;
        c.cut_feedrate = 800.0;
        c.plunge_feedrate = 300.0;
        c.spindle_speed = 1000.0;
    })
}

/// The reference Drill: 4 points, 1 mm tool, Standard, target −12, retract and clearance 3,
/// plunge 300, spindle 1000.
fn drill() -> Project {
    let shape = Shape::Points {
        points: vec![
            Point::new(34.0, 22.0),
            Point::new(53.0, 22.0),
            Point::new(53.0, 8.0),
            Point::new(34.0, 8.0),
        ],
    };
    job(shape, 1.0, Project::add_drill, |o| {
        let c = &mut o.common;
        c.target_depth = Some(-12.0);
        c.clearance_plane = 3.0;
        c.plunge_feedrate = 300.0;
        c.spindle_speed = 1000.0;
    })
}

/// The arc test shape (as `tests/shapes/square_convex_concave.dxf`): one closed polyline, drawn
/// anticlockwise, 100 × 60. Straight bottom and left edges with square corners, a convex arc on
/// the right (bulges out 15, R37.5) and a concave arc on the top (dips in 20, R72.5).
fn arc_shape() -> Shape {
    let v = |x: f64, y: f64, bulge: f64| Vertex { x, y, bulge };
    Shape::Polyline(Polyline {
        vertices: vec![
            v(0.0, 0.0, 0.0),
            v(100.0, 0.0, 0.5),
            v(100.0, 60.0, -0.4),
            v(0.0, 60.0, 0.0),
        ],
        closed: true,
    })
}

/// Settings shared by the arc test projects: 6 mm tool, target −5 in one pass, clearance 10,
/// feeds 1200 / 600, spindle 1000, Conventional.
fn arc_settings(o: &mut Operation) {
    let c = &mut o.common;
    c.target_depth = Some(-5.0);
    c.depth_increment = 5.0;
    c.clearance_plane = 10.0;
    c.cut_feedrate = 1200.0;
    c.plunge_feedrate = 600.0;
    c.spindle_speed = 1000.0;
    match &mut o.kind {
        OperationKind::Profile(p) => p.milling_direction = MillingDirection::Conventional,
        OperationKind::Pocket(p) => p.milling_direction = MillingDirection::Conventional,
        _ => {}
    }
}

fn arc_profile(side: ProfileSide) -> Project {
    job(arc_shape(), 6.0, Project::add_profile, |o| {
        arc_settings(o);
        if let OperationKind::Profile(p) = &mut o.kind {
            p.side = side;
        }
    })
}

/// Pocket of the arc shape, stepover 0.4.
fn arc_pocket() -> Project {
    job(arc_shape(), 6.0, Project::add_pocket, arc_settings)
}

fn programs() -> Vec<(&'static str, String)> {
    [
        ("Profile", profile()),
        ("Pocket", pocket()),
        ("Drill", drill()),
        ("Arcs Profile Outside", arc_profile(ProfileSide::Outside)),
        ("Arcs Profile Inside", arc_profile(ProfileSide::Inside)),
        ("Arcs Pocket", arc_pocket()),
    ]
    .into_iter()
    .map(|(name, mut p)| {
        let _ = p.save_text(name); // names the project like the file
        (name, p.gcode(DATE).unwrap().text())
    })
    .collect()
}

/// The lines compared: all but the version line, which changes with every release.
fn compared(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|l| !l.starts_with("( WHAM_CAM "))
        .collect()
}

#[test]
fn matches_the_checked_reference_gcode() {
    for (name, text) in programs() {
        let path = folder().join(format!("{name}.nc"));
        let want = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{name}: can't read {}: {e}", path.display()));
        assert_eq!(
            compared(&text),
            compared(&want),
            "{name}: the G-code differs from the checked file {}",
            path.display()
        );
    }
}

/// Each arc line of a program as (G2 or G3, radius from I J), rounded to 0.1.
fn arcs(text: &str) -> Vec<(String, f64)> {
    text.lines()
        .filter_map(|l| {
            let mut words = l.split_whitespace();
            let code = words.next()?;
            if code != "G2" && code != "G3" {
                return None;
            }
            let word = |c: char| {
                l.split_whitespace()
                    .find_map(|w| w.strip_prefix(c)?.parse::<f64>().ok())
                    .unwrap_or(0.0)
            };
            let r = word('I').hypot(word('J'));
            Some((code.to_string(), (r * 10.0).round() / 10.0))
        })
        .collect()
}

/// The codes used for arcs of radius `r` (expected to be one code only).
fn codes_for(arcs: &[(String, f64)], r: f64) -> Vec<&str> {
    let mut c: Vec<&str> = arcs
        .iter()
        .filter(|(_, ar)| (ar - r).abs() < 0.05)
        .map(|(c, _)| c.as_str())
        .collect();
    c.dedup();
    c
}

#[test]
fn conventional_outside_profile_cuts_convex_arcs_g3_and_concave_g2() {
    // Conventional outside runs anticlockwise. 6 mm tool, offset 3: convex arc 37.5 + 3, concave
    // arc 72.5 − 3, and tool-radius arcs (R3) round the square corners.
    let a = arcs(
        &arc_profile(ProfileSide::Outside)
            .gcode(DATE)
            .unwrap()
            .text(),
    );
    assert_eq!(codes_for(&a, 40.5), ["G3"], "convex arc: {a:?}");
    assert_eq!(codes_for(&a, 69.5), ["G2"], "concave arc: {a:?}");
    assert_eq!(codes_for(&a, 3.0), ["G3"], "corner arcs: {a:?}");
}

#[test]
fn conventional_inside_profile_cuts_convex_arcs_g2_and_concave_g3() {
    // Conventional inside runs clockwise. Convex arc 37.5 − 3, concave arc 72.5 + 3; the square
    // corners stay sharp (no corner arcs).
    let a = arcs(&arc_profile(ProfileSide::Inside).gcode(DATE).unwrap().text());
    assert_eq!(codes_for(&a, 34.5), ["G2"], "convex arc: {a:?}");
    assert_eq!(codes_for(&a, 75.5), ["G3"], "concave arc: {a:?}");
    assert_eq!(a.len(), 2, "only the two shape arcs: {a:?}");
}

#[test]
fn conventional_pocket_cuts_both_arc_directions() {
    // Rings run clockwise like a conventional inside profile: convex arcs G2, concave arcs G3.
    let a = arcs(&arc_pocket().gcode(DATE).unwrap().text());
    assert_eq!(codes_for(&a, 34.5), ["G2"], "outer ring convex arc: {a:?}");
    assert_eq!(codes_for(&a, 75.5), ["G3"], "outer ring concave arc: {a:?}");
}

#[test]
#[ignore = "writes files for the user to check; run on request"]
fn write_pending_reference_gcode() {
    let dir = folder().join("pending");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, text) in programs() {
        std::fs::write(dir.join(format!("{name}.nc")), text).unwrap();
    }
}
