//! Tooling: the master tool list, tool libraries, the tooling file, and picking a library tool
//! for an operation.

use wham_engine::tooling::{from_text, to_text};
use wham_engine::*;

fn tool(name: &str, end_type: EndType, diameter: f64) -> Tool {
    Tool {
        name: name.into(),
        end_type,
        diameter,
        auto_name: false,
        ..Tool::new_default()
    }
}

/// Master list: 6 mm flat (id 1), 3 mm flat (id 2), 6 mm ball (id 3); library "Mill" with
/// T1 = 6 mm flat, T5 = ball.
fn sample() -> Tooling {
    let mut t = Tooling::default();
    t.add_tool(tool("6mm 2F", EndType::Flat, 6.0)).unwrap();
    t.add_tool(tool("3mm 2F", EndType::Flat, 3.0)).unwrap();
    t.add_tool(tool("6mm ball", EndType::Ball, 6.0)).unwrap();
    let lib = t.add_library();
    t.rename_library(&lib, "Mill").unwrap();
    t.add_library_tool("Mill", 1, 1).unwrap();
    t.add_library_tool("Mill", 3, 5).unwrap();
    t
}

#[test]
fn tools_get_new_ids_and_are_checked() {
    let mut t = sample();
    assert_eq!(
        t.tools.iter().map(|x| x.id).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert!(t.add_tool(tool(" ", EndType::Flat, 6.0)).is_err());
    assert!(t.add_tool(tool("x", EndType::Flat, 0.0)).is_err());
    assert!(
        t.add_tool(Tool {
            flute_length: 70.0,
            ..Tool::new_default()
        })
        .is_err(),
        "flutes longer than the tool"
    );
    assert!(
        t.add_tool(Tool {
            end_type: EndType::VBit,
            v_angle: 180.0,
            ..Tool::new_default()
        })
        .is_err()
    );
    assert_eq!(t.tools.len(), 3, "refused tools are not added");
}

#[test]
fn automatic_names_follow_the_values() {
    let mut t = Tooling::default();
    let id = t.add_tool(Tool::new_default()).unwrap();
    assert_eq!(t.tool(id).unwrap().name, "6mm 2F Flat End Mill, 20mm flute");
    let mut edited = t.tool(id).unwrap().clone();
    edited.diameter = 3.175;
    edited.flutes = 1;
    edited.end_type = EndType::Ball;
    edited.flute_length = 12.5;
    t.set_tool(edited).unwrap();
    assert_eq!(
        t.tool(id).unwrap().name,
        "3.175mm 1F Ball Nose, 12.5mm flute"
    );
    let vbit = Tool {
        end_type: EndType::VBit,
        v_angle: 60.0,
        ..Tool::new_default()
    };
    assert_eq!(vbit.generated_name(), "6mm 60° 2F V-Bit, 20mm flute");
    let drill = Tool {
        end_type: EndType::Drill,
        ..Tool::new_default()
    };
    assert_eq!(drill.generated_name(), "6mm 2F Drill, 20mm flute");
}

#[test]
fn a_typed_name_is_kept() {
    let mut t = Tooling::default();
    let id = t
        .add_tool(Tool {
            name: "  My roughing cutter ".into(),
            auto_name: false,
            ..Tool::new_default()
        })
        .unwrap();
    let mut edited = t.tool(id).unwrap().clone();
    edited.diameter = 8.0;
    t.set_tool(edited).unwrap();
    assert_eq!(t.tool(id).unwrap().name, "My roughing cutter");
}

#[test]
fn tools_from_older_tooling_files_keep_their_names() {
    let mut file: serde_json::Value = serde_json::from_str(&to_text(&sample())).unwrap();
    for tool in file["tools"].as_array_mut().unwrap() {
        tool.as_object_mut().unwrap().remove("auto_name");
    }
    let text = file.to_string();
    assert!(!text.contains("auto_name"));
    let t = from_text(&text).unwrap();
    assert!(t.tools.iter().all(|x| !x.auto_name));
    assert_eq!(t.tools[0].name, "6mm 2F");
}

fn max_x(d: &ToolDrawing) -> f64 {
    d.outline.iter().map(|p| p.x).fold(f64::MIN, f64::max)
}

#[test]
fn drawing_of_a_flat_end_mill() {
    let t = Tool {
        shank_diameter: Some(8.0),
        ..Tool::new_default()
    };
    let d = t.drawing().unwrap();
    // Tip flat at y = 0, 3 mm either side; shank 4 mm either side up to the overall length.
    let has = |x: f64, y: f64| {
        d.outline
            .iter()
            .any(|p| (p.x - x).abs() < 1e-9 && (p.y - y).abs() < 1e-9)
    };
    assert!(has(3.0, 0.0) && has(-3.0, 0.0));
    assert!(has(3.0, 20.0) && has(4.0, 20.0) && has(4.0, 60.0) && has(-4.0, 60.0));
    assert_eq!(d.flute_top, 20.0);
    assert_eq!(max_x(&d), 4.0);
    // Symmetric about the tool's centre line.
    let min_x = d.outline.iter().map(|p| p.x).fold(f64::MAX, f64::min);
    assert!((min_x + 4.0).abs() < 1e-9);
    // Helix lines stay within the flutes.
    assert!(!d.flute_lines.is_empty());
    for (a, b) in &d.flute_lines {
        for p in [a, b] {
            assert!(p.y >= -1e-9 && p.y <= 20.0 + 1e-9 && p.x.abs() <= 3.0 + 1e-9);
        }
    }
}

#[test]
fn drawing_tips_by_end_type() {
    // V-bit 90°, 6 mm: point at the origin, full diameter 3 mm up.
    let v = Tool {
        end_type: EndType::VBit,
        ..Tool::new_default()
    }
    .drawing()
    .unwrap();
    assert!(v.outline.iter().any(|p| p.x == 0.0 && p.y == 0.0));
    assert!(
        v.outline
            .iter()
            .any(|p| (p.x - 3.0).abs() < 1e-9 && (p.y - 3.0).abs() < 1e-9)
    );
    // Ball: a half circle of radius 3, every tip point 3 from (0, 3).
    let b = Tool {
        end_type: EndType::Ball,
        ..Tool::new_default()
    }
    .drawing()
    .unwrap();
    let tip: Vec<_> = b.outline.iter().filter(|p| p.y < 3.0 - 1e-9).collect();
    assert!(tip.len() > 10);
    for p in tip {
        assert!((p.distance_to(Point::new(0.0, 3.0)) - 3.0).abs() < 1e-9);
    }
    // A V point longer than the flutes is cut off at the flute top.
    let long = Tool {
        end_type: EndType::VBit,
        v_angle: 20.0,
        flute_length: 5.0,
        ..Tool::new_default()
    }
    .drawing()
    .unwrap();
    assert!(long.outline.iter().all(|p| p.y >= 0.0));
    assert!(long.flute_top_radius < 3.0);
}

#[test]
fn no_drawing_for_invalid_values() {
    let t = Tool {
        diameter: 0.0,
        ..Tool::new_default()
    };
    assert!(t.drawing().is_none());
}

#[test]
fn shank_defaults_to_the_cutting_diameter() {
    let mut t = tool("x", EndType::Flat, 4.0);
    assert_eq!(t.shank(), 4.0);
    t.shank_diameter = Some(6.0);
    assert_eq!(t.shank(), 6.0);
    t.shank_diameter = Some(0.0);
    assert!(t.validate().is_err());
}

#[test]
fn editing_a_tool_keeps_its_id() {
    let mut t = sample();
    let mut edited = t.tool(2).unwrap().clone();
    edited.diameter = 3.175;
    t.set_tool(edited).unwrap();
    assert_eq!(t.tool(2).unwrap().diameter, 3.175);
    assert!(
        t.set_tool(Tool {
            id: 99,
            ..Tool::new_default()
        })
        .is_err()
    );
}

#[test]
fn deleting_a_master_tool_takes_it_out_of_libraries() {
    let mut t = sample();
    assert_eq!(t.libraries_using(3), vec!["Mill".to_string()]);
    assert!(t.delete_tool(3));
    assert!(t.tool(3).is_none());
    assert!(t.library_tool("Mill", 5).is_none());
    assert_eq!(t.library("Mill").unwrap().tools.len(), 1);
    assert!(!t.delete_tool(3));
}

#[test]
fn library_names_are_unique() {
    let mut t = sample();
    let second = t.add_library();
    assert_eq!(second, "Library 1");
    assert!(t.rename_library(&second, "Mill").is_err());
    assert!(t.rename_library(&second, "  ").is_err());
    assert_eq!(
        t.rename_library(&second, " Inch tooling ").unwrap(),
        "Inch tooling"
    );
    assert!(t.delete_library("Inch tooling"));
    assert!(!t.delete_library("Inch tooling"));
    assert_eq!(
        t.tools.len(),
        3,
        "deleting a library keeps the master tools"
    );
}

#[test]
fn tool_numbers_are_unique_in_a_library_and_start_at_1() {
    let mut t = sample();
    assert!(t.add_library_tool("Mill", 2, 1).is_err(), "T1 is taken");
    assert!(t.add_library_tool("Mill", 2, 0).is_err());
    assert!(t.add_library_tool("Mill", 42, 2).is_err(), "no such tool");
    assert!(t.add_library_tool("Nope", 2, 2).is_err());
    t.add_library_tool("Mill", 2, 3).unwrap();
    // Kept in number order.
    let numbers: Vec<u32> = t
        .library("Mill")
        .unwrap()
        .tools
        .iter()
        .map(|x| x.number)
        .collect();
    assert_eq!(numbers, vec![1, 3, 5]);
    // The same master tool can be in another library under another number.
    let other = t.add_library();
    t.add_library_tool(&other, 1, 12).unwrap();
    assert_eq!(t.library_tool(&other, 12).unwrap().name, "6mm 2F");
}

#[test]
fn renumbering_and_removing_library_tools() {
    let mut t = sample();
    assert!(
        t.set_library_tool_number("Mill", 1, 5).is_err(),
        "T5 is taken"
    );
    t.set_library_tool_number("Mill", 1, 7).unwrap();
    assert!(t.library_tool("Mill", 1).is_none());
    assert_eq!(t.library_tool("Mill", 7).unwrap().id, 1);
    assert!(t.remove_library_tool("Mill", 7));
    assert!(!t.remove_library_tool("Mill", 7));
    assert!(
        t.tool(1).is_some(),
        "removing from a library keeps the master tool"
    );
}

#[test]
fn tooling_file_round_trip() {
    let mut t = sample();
    t.tools[1].shank_diameter = Some(6.0);
    let text = to_text(&t);
    assert!(text.contains("\"format\": \"WHAM_CAM tooling\""));
    assert_eq!(from_text(&text).unwrap(), t);
}

#[test]
fn damaged_tooling_files_are_refused() {
    assert!(from_text("not json").is_err());
    assert!(from_text("{\"format\": \"something else\"}").is_err());
    let t = sample();
    let newer = to_text(&t).replace("\"format_version\": 1", "\"format_version\": 99");
    assert!(from_text(&newer).unwrap_err().contains("newer"));
    // A library pointing at a tool that isn't in the master list.
    let mut bad = t.clone();
    bad.libraries[0].tools[0].tool_id = 77;
    assert!(from_text(&to_text(&bad)).is_err());
    // Two tools with the same id.
    let mut bad = t.clone();
    bad.tools[1].id = 1;
    assert!(from_text(&to_text(&bad)).is_err());
}

// --- Operations -------------------------------------------------------------------------------

fn project_with_pocket() -> (Project, u32) {
    let mut proj = Project::new();
    let shape = proj
        .add_shape(
            "Default",
            Shape::rectangle(Point::new(0.0, 0.0), 50.0, 50.0, 0.0),
        )
        .unwrap();
    let op = proj.add_pocket(&[shape]).unwrap();
    (proj, op)
}

#[test]
fn picking_a_tool_copies_number_diameter_and_tool() {
    let t = sample();
    let (mut proj, op) = project_with_pocket();
    let picked = t.library_tool("Mill", 1).unwrap().clone();
    proj.set_operation_tool(op, Some((1, picked.clone())))
        .unwrap();
    let c = &proj.operation(op).unwrap().common;
    assert_eq!(c.tool_number, 1);
    assert_eq!(c.tool_diameter, 6.0);
    assert_eq!(c.tool.as_ref(), Some(&picked));
    // One undo step.
    proj.undo();
    assert!(proj.operation(op).unwrap().common.tool.is_none());
}

#[test]
fn only_flat_end_mills_can_be_picked() {
    let t = sample();
    let (mut proj, op) = project_with_pocket();
    let ball = t.library_tool("Mill", 5).unwrap().clone();
    let err = proj.set_operation_tool(op, Some((5, ball))).unwrap_err();
    assert!(err.contains("flat end mills only"), "{err}");
    assert!(proj.operation(op).unwrap().common.tool.is_none());
    assert!(!proj.can_undo() || proj.operation(op).unwrap().common.tool_number == 0);
}

#[test]
fn a_non_flat_tool_in_a_project_file_fails_to_generate() {
    // E.g. a hand-edited file: the engine still refuses to cut with it.
    let (mut proj, op) = project_with_pocket();
    let mut o = proj.operation(op).unwrap().clone();
    o.common.target_depth = Some(-1.0);
    o.common.tool = Some(tool("6mm ball", EndType::Ball, 6.0));
    proj.set_operation(o).unwrap();
    let report = proj.generate(&[op]);
    assert_eq!(report.generated, 0);
    assert!(report.errors[0].1.contains("flat end mills only"));
}

#[test]
fn typing_a_different_diameter_drops_the_tool_copy() {
    let t = sample();
    let (mut proj, op) = project_with_pocket();
    proj.set_operation_tool(op, Some((1, t.library_tool("Mill", 1).unwrap().clone())))
        .unwrap();
    // Changing something else keeps the tool.
    let mut o = proj.operation(op).unwrap().clone();
    o.common.cut_feedrate = 1500.0;
    proj.set_operation(o).unwrap();
    assert!(proj.operation(op).unwrap().common.tool.is_some());
    // Typing a new diameter drops it (number and diameter stay as typed).
    let mut o = proj.operation(op).unwrap().clone();
    o.common.tool_diameter = 5.0;
    proj.set_operation(o).unwrap();
    let c = &proj.operation(op).unwrap().common;
    assert!(c.tool.is_none());
    assert_eq!((c.tool_number, c.tool_diameter), (1, 5.0));
}

#[test]
fn clearing_the_tool_keeps_number_and_diameter() {
    let t = sample();
    let (mut proj, op) = project_with_pocket();
    proj.set_operation_tool(op, Some((1, t.library_tool("Mill", 1).unwrap().clone())))
        .unwrap();
    proj.set_operation_tool(op, None).unwrap();
    let c = &proj.operation(op).unwrap().common;
    assert!(c.tool.is_none());
    assert_eq!((c.tool_number, c.tool_diameter), (1, 6.0));
}

#[test]
fn tool_copy_and_active_library_are_saved_in_the_project() {
    let t = sample();
    let (mut proj, op) = project_with_pocket();
    proj.set_active_library(Some("Mill".into()));
    proj.set_operation_tool(op, Some((1, t.library_tool("Mill", 1).unwrap().clone())))
        .unwrap();
    let text = proj.save_text("test");
    // Opened on "another computer": no tooling needed.
    let mut other = Project::new();
    other.open(&text, "test").unwrap();
    assert_eq!(other.document().active_library.as_deref(), Some("Mill"));
    let c = &other.operation(op).unwrap().common;
    assert_eq!(c.tool.as_ref().unwrap().name, "6mm 2F");
    assert_eq!((c.tool_number, c.tool_diameter), (1, 6.0));
}

#[test]
fn older_project_files_open_without_tool_fields() {
    let (mut proj, op) = project_with_pocket();
    let mut file: serde_json::Value = serde_json::from_str(&proj.save_text("old")).unwrap();
    let doc = file["document"].as_object_mut().unwrap();
    doc.remove("active_library");
    doc["parts"][0]["operations"][0]["common"]
        .as_object_mut()
        .unwrap()
        .remove("tool");
    let old = file.to_string();
    assert!(!old.contains("active_library") && !old.contains("\"tool\""));
    let mut p = Project::new();
    p.open(&old, "old").unwrap();
    assert!(p.document().active_library.is_none());
    assert!(p.operation(op).unwrap().common.tool.is_none());
}

#[test]
fn renaming_a_library_follows_in_the_project() {
    let (mut proj, _) = project_with_pocket();
    proj.set_active_library(Some("Mill".into()));
    proj.library_renamed("Mill", "Router");
    assert_eq!(proj.document().active_library.as_deref(), Some("Router"));
    proj.undo();
    assert_eq!(proj.document().active_library, None);
    proj.redo();
    assert_eq!(proj.document().active_library.as_deref(), Some("Router"));
}
