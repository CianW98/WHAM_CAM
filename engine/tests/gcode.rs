//! G-code output (spec 7): the generic post-processor's format, the read-back check, and
//! Project::gcode's refusals.

use wham_engine::gcode_read::check_program;
use wham_engine::operation::{DrillMethod, OperationKind, SpindleDirection};
use wham_engine::post::{Header, JobOperation, PostFormat, comment, number, write_program};
use wham_engine::toolpath::{Limits, Move, Point3, Toolpath};
use wham_engine::*;

const DATE: &str = "2026-11-20 10:00";

fn tool(number: u32, diameter: f64) -> PickedTool {
    PickedTool {
        number,
        tool: Tool {
            diameter,
            ..Tool::new_default()
        },
    }
}

/// A project with a 6 mm Machining tool (T1).
fn project() -> Project {
    let mut p = Project::new();
    p.set_machining_tool(Some(tool(1, 6.0))).unwrap();
    p
}

fn set_depth(p: &mut Project, op: u32, target: f64, edit: impl FnOnce(&mut Operation)) {
    let mut o = p.operation(op).unwrap().clone();
    o.common.target_depth = Some(target);
    edit(&mut o);
    p.set_operation(o).unwrap();
}

fn lines(p: &Project) -> Vec<String> {
    p.gcode(DATE).unwrap().lines
}

fn square(x: f64, y: f64, size: f64) -> Shape {
    Shape::rectangle(Point::new(x, y), size, size, 0.0)
}

// --- Number and comment format ------------------------------------------------------------------

#[test]
fn numbers_have_up_to_3_decimals_with_trailing_zeros_trimmed() {
    assert_eq!(number(12.5, 3), "12.5");
    assert_eq!(number(3.0, 3), "3");
    assert_eq!(number(-0.25, 3), "-0.25");
    assert_eq!(number(1.23456, 3), "1.235");
    assert_eq!(number(-0.0001, 3), "0");
    assert_eq!(number(1200.0, 0), "1200");
}

#[test]
fn comments_cant_be_closed_early() {
    assert_eq!(comment("Pocket (left)"), "( Pocket [left] )");
    assert_eq!(comment("two\nlines"), "( two lines )");
}

// --- The whole program --------------------------------------------------------------------------

#[test]
fn a_pocket_program_has_the_decided_start_tool_spindle_and_end() {
    let mut p = project();
    let shape = p.add_shape("Default", square(0.0, 0.0, 30.0)).unwrap();
    let op = p.add_pocket(&[shape]).unwrap();
    set_depth(&mut p, op, -2.0, |_| {});
    assert_eq!(p.generate(&[]).generated, 1);
    let l = lines(&p);
    assert_eq!(l[0], format!("( WHAM_CAM {} )", env!("CARGO_PKG_VERSION")));
    assert_eq!(l[1], format!("( Project: Untitled   {DATE} )"));
    assert_eq!(l[2], "( Post-processor: Generic   Machine: none )");
    assert_eq!(l[3], "( T1: 6mm 2F Spiral End Mill, 20mm flute, D6 )");
    assert_eq!(l[4], "G21 G90 G17");
    assert_eq!(l[5], "G0 Z3");
    assert_eq!(
        l[6],
        "( Pocket1: Pocket, T1 D6, stock 0, target -2, depth increment 1 )"
    );
    assert_eq!(l[7], "( T1: 6mm 2F Spiral End Mill, 20mm flute )");
    assert_eq!(l[8], "T1 M6");
    assert_eq!(l[9], "S10000 M3");
    assert_eq!(l[10], "G4 P2");
    assert!(
        l[11].starts_with("G0 X") && l[11].contains(" Y"),
        "{}",
        l[11]
    );
    assert_eq!(&l[l.len() - 2..], ["M5", "M30"]);
    // Only words that change; F only when the feed changes.
    let feeds = l.iter().filter(|x| x.starts_with("G1")).count();
    let f_words = l.iter().filter(|x| x.contains(" F")).count();
    assert!(f_words < feeds, "F is written only when it changes");
    assert!(!l.iter().any(|x| x.contains("X") && x.starts_with("G0 Z")));
}

#[test]
fn profile_corners_are_arcs_with_i_and_j() {
    let mut p = project();
    let shape = p.add_shape("Default", square(0.0, 0.0, 20.0)).unwrap();
    let op = p.add_profile(&[shape]).unwrap();
    set_depth(&mut p, op, -1.0, |_| {});
    p.generate(&[]);
    let l = lines(&p);
    let arcs: Vec<&String> = l
        .iter()
        .filter(|x| matches!(x.split_whitespace().next(), Some("G2" | "G3")))
        .collect();
    // Four outside corners; the cut starts partway round one, so that corner is two arcs.
    assert!(arcs.len() >= 4, "outside corners: {arcs:#?}");
    for a in arcs {
        assert!(
            a.contains(" X") && a.contains(" Y") && a.contains(" I") && a.contains(" J"),
            "{a}"
        );
    }
}

#[test]
fn drilling_uses_g81_like_the_reference() {
    // The reference Drill example: 4 points, 1 mm tool, target −12, R 3, plunge 300.
    let mut p = project();
    let shape = p
        .add_shape(
            "Default",
            Shape::Points {
                points: vec![
                    Point::new(34.0, 22.0),
                    Point::new(53.0, 22.0),
                    Point::new(53.0, 8.0),
                    Point::new(34.0, 8.0),
                ],
            },
        )
        .unwrap();
    let op = p.add_drill(&[shape]).unwrap();
    set_depth(&mut p, op, -12.0, |_| {});
    p.generate(&[]);
    let l = lines(&p);
    let start = l.iter().position(|x| x.starts_with("G98")).unwrap();
    assert_eq!(
        &l[start..start + 5],
        [
            "G98 G81 X34 Y22 Z-12 R3 F300",
            "G81 Y8",
            "G81 X53",
            "G81 Y22",
            "G80"
        ]
    );
}

#[test]
fn peck_drilling_uses_g83_with_q() {
    let mut p = project();
    let shape = p
        .add_shape(
            "Default",
            Shape::Points {
                points: vec![Point::new(5.0, 5.0)],
            },
        )
        .unwrap();
    let op = p.add_drill(&[shape]).unwrap();
    set_depth(&mut p, op, -5.0, |o| {
        if let OperationKind::Drill(d) = &mut o.kind {
            d.method = DrillMethod::Peck;
            d.peck_distance = 2.0;
        }
    });
    p.generate(&[]);
    let l = lines(&p);
    assert!(
        l.contains(&"G98 G83 X5 Y5 Z-5 R3 Q2 F300".to_string()),
        "{l:#?}"
    );
    assert!(l.contains(&"G80".to_string()));
}

#[test]
fn spiral_drilling_is_written_out_as_a_helix() {
    let mut p = project();
    let shape = p
        .add_shape(
            "Default",
            Shape::Circle {
                center: Point::new(0.0, 0.0),
                radius: 5.0,
            },
        )
        .unwrap();
    let op = p.add_drill(&[shape]).unwrap();
    set_depth(&mut p, op, -3.0, |o| {
        if let OperationKind::Drill(d) = &mut o.kind {
            d.method = DrillMethod::Spiral;
        }
    });
    p.generate(&[]);
    let l = lines(&p);
    assert!(!l.iter().any(|x| x.contains("G81") || x.contains("G83")));
    assert!(
        l.iter()
            .any(|x| x.split_whitespace().next() == Some("G2") && x.contains(" Z")),
        "a helix arc goes down"
    );
}

#[test]
fn tool_changes_and_clearance_lifts_between_operations() {
    let mut p = project();
    let a = p.add_shape("Default", square(0.0, 0.0, 30.0)).unwrap();
    let b = p.add_shape("Default", square(50.0, 0.0, 30.0)).unwrap();
    let c = p.add_shape("Default", square(100.0, 0.0, 30.0)).unwrap();
    let op1 = p.add_pocket(&[a]).unwrap();
    let op2 = p.add_pocket(&[b]).unwrap();
    let op3 = p.add_pocket(&[c]).unwrap();
    set_depth(&mut p, op1, -1.0, |_| {});
    // op2: same tool, higher clearance plane, slower spindle.
    set_depth(&mut p, op2, -1.0, |o| {
        o.common.clearance_plane = 10.0;
        o.common.spindle_speed = 8000.0;
    });
    // op3: another tool.
    set_depth(&mut p, op3, -1.0, |_| {});
    p.set_operation_tool(op3, ToolSetting::Library(tool(2, 3.0)))
        .unwrap();
    assert_eq!(p.generate(&[]).generated, 3);
    let l = lines(&p);
    let text = l.join("\n");
    assert_eq!(text.matches("M6").count(), 2, "T1 once, T2 once");
    // Before op2: straight up to its clearance plane, then the new speed.
    let at = l.iter().position(|x| x.starts_with("( Pocket2")).unwrap();
    assert_eq!(l[at + 1], "G0 Z10");
    assert_eq!(l[at + 2], "S8000 M3");
    // Before T2: spindle off first.
    let t2 = l.iter().position(|x| x == "T2 M6").unwrap();
    assert!(l[t2 - 2] == "M5", "{:#?}", &l[t2 - 3..=t2]);
    assert!(text.contains("( T2: 6mm 2F Spiral End Mill, 20mm flute, D3 )"));
}

#[test]
fn spindle_off_operations_write_m5() {
    let mut p = project();
    let a = p.add_shape("Default", square(0.0, 0.0, 30.0)).unwrap();
    let op = p.add_pocket(&[a]).unwrap();
    set_depth(&mut p, op, -1.0, |o| {
        o.common.spindle_direction = SpindleDirection::Off
    });
    p.generate(&[]);
    let l = lines(&p);
    let words: Vec<&str> = l.iter().flat_map(|x| x.split_whitespace()).collect();
    assert!(!words.contains(&"M3") && !words.contains(&"M4"));
}

// --- Refusals -----------------------------------------------------------------------------------

#[test]
fn nothing_to_write_or_toolpaths_not_ready_are_refused() {
    let mut p = project();
    assert!(p.gcode(DATE).unwrap_err().contains("Nothing to write"));
    let a = p.add_shape("Default", square(0.0, 0.0, 30.0)).unwrap();
    let op = p.add_pocket(&[a]).unwrap();
    assert!(
        p.gcode(DATE)
            .unwrap_err()
            .contains("Pocket1 (not generated)")
    );
    set_depth(&mut p, op, -1.0, |_| {});
    p.generate(&[]);
    assert!(p.gcode(DATE).is_ok());
    // Changed since: out of date.
    set_depth(&mut p, op, -2.0, |_| {});
    let err = p.gcode(DATE).unwrap_err();
    assert!(
        err.contains("Press Generate toolpaths") && err.contains("Pocket1 (out of date)"),
        "{err}"
    );
    // Disabled operations are left out.
    let mut o = p.operation(op).unwrap().clone();
    o.enabled = false;
    p.set_operation(o).unwrap();
    assert!(p.gcode(DATE).unwrap_err().contains("Nothing to write"));
}

// --- The read-back check catches mistakes ------------------------------------------------------

fn sample_toolpath() -> Toolpath {
    let r = |x, y, z| Move::Rapid {
        to: Point3::new(x, y, z),
    };
    let f = |x, y, z| Move::Feed {
        to: Point3::new(x, y, z),
        feed: 500.0,
    };
    Toolpath {
        operation_id: 1,
        moves: vec![
            r(0.0, 0.0, 3.0),
            r(0.0, 0.0, 1.0),
            f(0.0, 0.0, -1.0),
            f(10.0, 0.0, -1.0),
            Move::Arc {
                to: Point3::new(10.0, 10.0, -1.0),
                center_x: 10.0,
                center_y: 5.0,
                ccw: true,
                feed: 500.0,
            },
            r(10.0, 10.0, 3.0),
        ],
        limits: Limits {
            clearance_plane: 3.0,
            stock_surface: 0.0,
            target_depth: -1.0,
        },
        cycles: Vec::new(),
    }
}

fn sample_job(tp: &Toolpath) -> Vec<JobOperation<'_>> {
    vec![JobOperation {
        name: "Op".into(),
        kind: "Profile".into(),
        tool_number: 1,
        tool_name: "6mm".into(),
        tool_diameter: 6.0,
        spindle: SpindleDirection::CW,
        spindle_speed: 10000.0,
        stock_surface: 0.0,
        target_depth: -1.0,
        depth_increment: Some(1.0),
        fast_plunge_height: 1.0,
        toolpath: tp,
    }]
}

fn header() -> Header {
    Header {
        project: "t".into(),
        date: DATE.into(),
        machine: None,
        post: "Generic".into(),
    }
}

#[test]
fn a_correct_program_passes_the_read_back_check() {
    let tp = sample_toolpath();
    let job = sample_job(&tp);
    let prog = write_program(&header(), &job, &PostFormat::generic());
    assert_eq!(check_program(&prog, &job, 3), Vec::<String>::new());
    assert!(
        prog.lines.contains(&"G3 X10 Y10 I0 J5".to_string()),
        "{:#?}",
        prog.lines
    );
}

#[test]
fn a_changed_line_is_caught() {
    let tp = sample_toolpath();
    let job = sample_job(&tp);
    let good = write_program(&header(), &job, &PostFormat::generic());
    let try_change = |from: &str, to: &str| {
        let mut prog = good.clone();
        let i = prog
            .lines
            .iter()
            .position(|l| l == from)
            .unwrap_or_else(|| panic!("{from}"));
        prog.lines[i] = to.to_string();
        check_program(&prog, &job, 3)
    };
    // A wrong coordinate, a rapid instead of a feed, an arc the wrong way, a bad arc radius.
    assert!(!try_change("G1 X10", "G1 X10.5").is_empty());
    assert!(!try_change("G1 X10", "G0 X10").is_empty());
    assert!(!try_change("G3 X10 Y10 I0 J5", "G2 X10 Y10 I0 J5").is_empty());
    assert!(!try_change("G3 X10 Y10 I0 J5", "G3 X10 Y10.01 I0 J5").is_empty());
    // The spindle never started.
    assert!(!try_change("S10000 M3", "S10000").is_empty());
    // No M30 at the end.
    let mut prog = good.clone();
    prog.lines.pop();
    assert!(!check_program(&prog, &job, 3).is_empty());
    // A code we never write.
    assert!(!try_change("G21 G90 G17", "G20 G90 G17").is_empty());
}
