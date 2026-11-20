//! Known-good G-code (spec 8): the three reference projects (Profile, Pocket, Drill, as in the
//! reference examples) written by the generic post-processor, compared exactly with the files in
//! `tests/reference/wham/` once the user has checked them in a simulator and with an air cut.
//!
//! That folder is on this computer only (git-ignored), so on GitHub these comparisons are skipped.
//! To write fresh files for checking, run:
//!   cargo test -p wham_engine --test reference_gcode -- --ignored
//! which writes them to `tests/reference/wham/pending/`. After the user has checked them, they are
//! moved up into `tests/reference/wham/`.

use std::path::PathBuf;

use wham_engine::geometry::{Polyline, Vertex};
use wham_engine::*;

/// A fixed date, so the files don't change from day to day.
const DATE: &str = "reference";

fn folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/reference/wham")
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

fn programs() -> Vec<(&'static str, String)> {
    [
        ("Profile", profile()),
        ("Pocket", pocket()),
        ("Drill", drill()),
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
        let Ok(want) = std::fs::read_to_string(&path) else {
            eprintln!(
                "{} isn't here (it is kept on the user's computer only); skipped.",
                path.display()
            );
            continue;
        };
        assert_eq!(
            compared(&text),
            compared(&want),
            "{name}: the G-code differs from the checked file {}",
            path.display()
        );
    }
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
