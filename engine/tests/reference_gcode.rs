//! Known-good G-code (spec 8): the reference projects (Profile, Pocket and Drill on the test
//! shapes) written by the generic post-processor, compared exactly with the checked files in
//! `engine/tests/reference_gcode/` (see the README there).
//!
//! Every project uses the default settings (clearance 3, feeds 800 / 300, spindle 10000, target
//! −10) with a 6 mm end mill as T2, except where a project says otherwise.
//!
//! To write fresh files for checking, run:
//!   cargo test -p wham_engine --test reference_gcode -- --ignored
//! which writes them to `engine/tests/reference_gcode/pending/` (git-ignored). After the user has
//! checked them in a simulator and with an air cut, they are moved up into the folder itself.

use std::path::PathBuf;

use wham_engine::geometry::{Polyline, Vertex};
use wham_engine::operation::{
    CutOrdering, DrillMethod, MillingDirection, OperationKind, ProfileSide,
};
use wham_engine::*;

/// A fixed date, so the files don't change from day to day.
const DATE: &str = "reference";

fn folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/reference_gcode")
}

/// A `diameter` mm, `flute` mm flute end mill as tool `number`.
fn end_mill(number: u32, diameter: f64, flute: f64) -> PickedTool {
    PickedTool {
        number,
        tool: Tool {
            diameter,
            flute_length: flute,
            ..Tool::new_default()
        }
        .named(),
    }
}

/// A new project with the 6 mm end mill as T2 for Machining.
fn new_project() -> Project {
    let mut p = Project::new();
    p.set_machining_tool(Some(end_mill(2, 6.0, 20.0))).unwrap();
    p
}

/// Adds an operation from `add` on `shapes`, with target −10 and its settings changed by `edit`.
fn add_op(
    p: &mut Project,
    shapes: &[u32],
    add: fn(&mut Project, &[u32]) -> Result<u32, String>,
    edit: impl FnOnce(&mut Operation),
) -> u32 {
    let op = add(p, shapes).unwrap();
    let mut o = p.operation(op).unwrap().clone();
    o.common.target_depth = Some(-10.0);
    edit(&mut o);
    p.set_operation(o).unwrap();
    op
}

/// Generates every operation, checking nothing failed.
fn generated(mut p: Project) -> Project {
    let report = p.generate(&[]);
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    p
}

/// A project with one operation from `add` on `shape`.
fn job(
    shape: Shape,
    add: fn(&mut Project, &[u32]) -> Result<u32, String>,
    edit: impl FnOnce(&mut Operation),
) -> Project {
    let mut p = new_project();
    let id = p.add_shape("Default", shape).unwrap();
    add_op(&mut p, &[id], add, edit);
    generated(p)
}

/// The arc test shape (as `tests/shapes/square_convex_concave.dxf`): one closed polyline, drawn
/// anticlockwise, 100 × 60. Straight bottom and left edges with square corners, a convex arc on
/// the right (bulges out 15, R37.5) and a concave arc on the top (dips in 20, R72.5).
fn arc_shape() -> Shape {
    arc_polyline(true)
}

/// The arc test shape, closed or open (open: without the left edge from (0, 60) back to (0, 0)).
fn arc_polyline(closed: bool) -> Shape {
    let v = |x: f64, y: f64, bulge: f64| Vertex { x, y, bulge };
    Shape::Polyline(Polyline {
        vertices: vec![
            v(0.0, 0.0, 0.0),
            v(100.0, 0.0, 0.5),
            v(100.0, 60.0, -0.4),
            v(0.0, 60.0, 0.0),
        ],
        closed,
    })
}

/// A closed rectangle from (x0, y0) to (x1, y1).
fn rectangle(x0: f64, y0: f64, x1: f64, y1: f64) -> Shape {
    Shape::Polyline(Polyline {
        vertices: vec![
            Vertex::line(x0, y0),
            Vertex::line(x1, y0),
            Vertex::line(x1, y1),
            Vertex::line(x0, y1),
        ],
        closed: true,
    })
}

/// Profile of the arc shape on `side`, Conventional, in one 10 mm pass.
fn arc_profile(side: ProfileSide) -> Project {
    profile_of(arc_shape(), side, MillingDirection::Conventional)
}

/// Profile of `shape` on `side` in `direction`, in one 10 mm pass.
fn profile_of(shape: Shape, side: ProfileSide, direction: MillingDirection) -> Project {
    job(shape, Project::add_profile, |o| {
        o.common.depth_increment = 10.0;
        if let OperationKind::Profile(p) = &mut o.kind {
            p.side = side;
            p.milling_direction = direction;
        }
    })
}

/// Pocket of the arc shape, stepover 0.4, in one 10 mm pass.
fn arc_pocket(direction: MillingDirection) -> Project {
    job(arc_shape(), Project::add_pocket, |o| {
        o.common.depth_increment = 10.0;
        if let OperationKind::Pocket(p) = &mut o.kind {
            p.milling_direction = direction;
        }
    })
}

/// The two pocket rectangles, 100 × 50: one at the bottom, one above it from Y70.
fn two_rectangles(p: &mut Project) -> (u32, u32) {
    let bottom = p
        .add_shape("Default", rectangle(0.0, 0.0, 100.0, 50.0))
        .unwrap();
    let top = p
        .add_shape("Default", rectangle(0.0, 70.0, 100.0, 120.0))
        .unwrap();
    (bottom, top)
}

/// One pocket of both rectangles, Level first, Conventional, in 5 mm passes.
fn pocket_level_first() -> Project {
    let mut p = new_project();
    let (bottom, top) = two_rectangles(&mut p);
    add_op(&mut p, &[bottom, top], Project::add_pocket, |o| {
        o.common.depth_increment = 5.0;
        if let OperationKind::Pocket(k) = &mut o.kind {
            k.cut_ordering = CutOrdering::LevelFirst;
        }
    });
    generated(p)
}

/// Two pockets in 5 mm passes: the top rectangle with T2 (6 mm), then the bottom one with its
/// own tool T1 (a 3 mm, 15 mm flute end mill), so the program changes tool between them.
fn pocket_tool_change() -> Project {
    let mut p = new_project();
    let (bottom, top) = two_rectangles(&mut p);
    add_op(&mut p, &[top], Project::add_pocket, |o| {
        o.common.depth_increment = 5.0;
    });
    let second = add_op(&mut p, &[bottom], Project::add_pocket, |o| {
        o.common.depth_increment = 5.0;
    });
    p.set_operation_tool(second, ToolSetting::Library(end_mill(1, 3.0, 15.0)))
        .unwrap();
    generated(p)
}

/// Drilling 4 points with `method`: peck 2 (Peck); hole 8 mm, 2 mm down per turn (Helical).
fn drill(method: DrillMethod) -> Project {
    let shape = Shape::Points {
        points: vec![
            Point::new(50.0, 150.0),
            Point::new(150.0, 150.0),
            Point::new(150.0, 50.0),
            Point::new(50.0, 50.0),
        ],
    };
    job(shape, Project::add_drill, |o| {
        o.common.depth_increment = 2.0;
        if let OperationKind::Drill(d) = &mut o.kind {
            d.method = method;
            d.peck_distance = 2.0;
            d.hole_diameter = 8.0;
        }
    })
}

fn programs() -> Vec<(&'static str, String)> {
    [
        (
            "Profile_Outside_Conventional",
            arc_profile(ProfileSide::Outside),
        ),
        (
            "Profile_Inside_Conventional",
            arc_profile(ProfileSide::Inside),
        ),
        (
            "Profile_On_Line_Conventional",
            arc_profile(ProfileSide::OnLine),
        ),
        (
            "Profile_Outside_Climb",
            profile_of(arc_shape(), ProfileSide::Outside, MillingDirection::Climb),
        ),
        (
            "Profile_Inside_Climb",
            profile_of(arc_shape(), ProfileSide::Inside, MillingDirection::Climb),
        ),
        (
            "Profile_On_Line_Climb",
            profile_of(arc_shape(), ProfileSide::OnLine, MillingDirection::Climb),
        ),
        (
            "Profile_On_Line_Conventional_Open",
            profile_of(
                arc_polyline(false),
                ProfileSide::OnLine,
                MillingDirection::Conventional,
            ),
        ),
        (
            "Pocket_Conventional",
            arc_pocket(MillingDirection::Conventional),
        ),
        ("Pocket_Climb", arc_pocket(MillingDirection::Climb)),
        ("Pocket_Level_First_Conventional", pocket_level_first()),
        ("Pocket_Tool_Change_Conventional", pocket_tool_change()),
        ("Drill_Standard", drill(DrillMethod::Standard)),
        ("Drill_Peck", drill(DrillMethod::Peck)),
        ("Drill_Helical", drill(DrillMethod::Helical)),
    ]
    .into_iter()
    .map(|(name, p)| (name, p.gcode(DATE).unwrap().text()))
    .collect()
}

/// The lines compared: all but the version line (changes with every release) and the project
/// line (the project's name and when the file was made).
fn compared(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|l| !l.starts_with("( WHAM_CAM ") && !l.starts_with("( Project: "))
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
    let a = arcs(
        &arc_pocket(MillingDirection::Conventional)
            .gcode(DATE)
            .unwrap()
            .text(),
    );
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
