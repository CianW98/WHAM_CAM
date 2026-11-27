//! Shared settings follow the Machining tree (spec 5.4): Machining → part → operation, each level
//! able to set its own values; and parts (new, rename, delete, move an operation).

use wham_engine::operation::{MillingDirection, SharedSetting, SharedSettings, SpindleDirection};
use wham_engine::*;

fn end_mill(number: u32, diameter: f64) -> PickedTool {
    PickedTool {
        number,
        tool: Tool {
            diameter,
            ..Tool::new_default()
        }
        .named(),
    }
}

fn circle(r: f64) -> Shape {
    Shape::Circle {
        center: Point::new(0.0, 0.0),
        radius: r,
    }
}

/// A project with a 6 mm end mill on Machining and one pocket (target −5) on a circle.
fn with_pocket() -> (Project, u32) {
    let mut p = Project::new();
    p.set_machining_tool(Some(end_mill(1, 6.0))).unwrap();
    let shape = p.add_shape("Default", circle(20.0)).unwrap();
    let id = p.add_pocket(&[shape]).unwrap();
    let mut op = p.operation(id).unwrap().clone();
    op.common.target_depth = Some(-5.0);
    p.set_operation(op).unwrap();
    (p, id)
}

/// Non-default values for every shared setting.
fn changed() -> SharedSettings {
    SharedSettings {
        stock_surface: 1.0,
        depth_increment: 2.0,
        final_depth_increment: 0.5,
        clearance_plane: 6.0,
        fast_plunge_height: 2.0,
        cut_feedrate: 1200.0,
        plunge_feedrate: 400.0,
        spindle_speed: 18000.0,
        spindle_direction: SpindleDirection::CCW,
        roughing_clearance: 0.2,
        milling_direction: MillingDirection::Climb,
    }
}

fn gcode(p: &mut Project) -> String {
    let report = p.generate(&[]);
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    p.gcode("2026-01-01").unwrap().text()
}

fn status(p: &Project, id: u32) -> String {
    p.operation_states()
        .into_iter()
        .find(|s| s.id == id)
        .unwrap()
        .status
}

#[test]
fn a_new_operation_follows_machining_in_everything() {
    let (p, id) = with_pocket();
    let op = p.operation(id).unwrap();
    // Only the target depth was set; it isn't a shared setting.
    assert!(op.own.is_empty());
    assert_eq!(op.shared(), SharedSettings::default());
}

#[test]
fn following_machining_gives_the_same_gcode_as_the_same_values_set_on_the_operation() {
    let (mut followed, _) = with_pocket();
    followed.set_machining_settings(changed()).unwrap();

    let (mut own, id) = with_pocket();
    let mut op = own.operation(id).unwrap().clone();
    op.set_shared(&changed());
    own.set_operation(op).unwrap();
    assert_eq!(
        own.operation(id).unwrap().own.len(),
        SharedSetting::ALL.len()
    );

    assert_eq!(gcode(&mut followed), gcode(&mut own));
}

#[test]
fn a_machining_change_reaches_following_operations_and_marks_them_out_of_date() {
    let (mut p, id) = with_pocket();
    p.generate(&[]);
    assert_eq!(status(&p, id), "ok");
    let s = SharedSettings {
        clearance_plane: 10.0,
        ..Default::default()
    };
    p.set_machining_settings(s).unwrap();
    assert_eq!(p.operation(id).unwrap().common.clearance_plane, 10.0);
    assert_eq!(status(&p, id), "outdated");
}

#[test]
fn an_own_value_stays_until_reset() {
    let (mut p, id) = with_pocket();
    let mut op = p.operation(id).unwrap().clone();
    op.common.cut_feedrate = 1500.0;
    p.set_operation(op).unwrap();
    assert_eq!(
        p.operation(id)
            .unwrap()
            .own
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![SharedSetting::CutFeedrate]
    );

    let s = SharedSettings {
        cut_feedrate: 900.0,
        plunge_feedrate: 250.0,
        ..Default::default()
    };
    p.set_machining_settings(s).unwrap();
    let op = p.operation(id).unwrap();
    assert_eq!(op.common.cut_feedrate, 1500.0);
    assert_eq!(op.common.plunge_feedrate, 250.0);

    p.reset_operation_setting(id, SharedSetting::CutFeedrate)
        .unwrap();
    let op = p.operation(id).unwrap();
    assert!(op.own.is_empty());
    assert_eq!(op.common.cut_feedrate, 900.0);
}

#[test]
fn milling_direction_follows_into_the_operation_type() {
    let (mut p, id) = with_pocket();
    let s = SharedSettings {
        milling_direction: MillingDirection::Climb,
        ..Default::default()
    };
    p.set_machining_settings(s).unwrap();
    assert_eq!(
        p.operation(id).unwrap().kind.milling_direction(),
        MillingDirection::Climb
    );
}

#[test]
fn a_part_sits_between_machining_and_its_operations() {
    let (mut p, id) = with_pocket();
    let mut part = p.document().parts[0].settings.clone();
    part.spindle_speed = 15000.0;
    p.set_part_settings(0, part).unwrap();
    assert_eq!(
        p.document().parts[0]
            .own
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![SharedSetting::SpindleSpeed]
    );

    let s = SharedSettings {
        spindle_speed: 12000.0,
        cut_feedrate: 1000.0,
        ..Default::default()
    };
    p.set_machining_settings(s).unwrap();
    let op = p.operation(id).unwrap();
    assert_eq!(op.common.spindle_speed, 15000.0, "the part's own value");
    assert_eq!(op.common.cut_feedrate, 1000.0, "Machining's value");

    // The operation's own value beats the part's.
    let mut op = op.clone();
    op.common.spindle_speed = 9000.0;
    p.set_operation(op).unwrap();
    assert_eq!(p.operation(id).unwrap().common.spindle_speed, 9000.0);

    p.reset_part_setting(0, SharedSetting::SpindleSpeed)
        .unwrap();
    assert_eq!(p.document().parts[0].settings.spindle_speed, 12000.0);
    assert_eq!(p.operation(id).unwrap().common.spindle_speed, 9000.0);
}

#[test]
fn settings_that_make_no_sense_are_refused() {
    let (mut p, _) = with_pocket();
    let s = SharedSettings {
        clearance_plane: -1.0,
        ..Default::default()
    };
    assert!(p.set_machining_settings(s.clone()).is_err());
    assert!(p.set_part_settings(0, s).is_err());
    let s = SharedSettings {
        cut_feedrate: 0.0,
        ..Default::default()
    };
    assert!(p.set_machining_settings(s).is_err());
    assert_eq!(p.document().machining_settings, SharedSettings::default());
}

#[test]
fn shared_setting_changes_are_undo_steps() {
    let (mut p, id) = with_pocket();
    let s = SharedSettings {
        depth_increment: 3.0,
        ..Default::default()
    };
    p.set_machining_settings(s).unwrap();
    assert_eq!(p.operation(id).unwrap().common.depth_increment, 3.0);
    p.undo();
    assert_eq!(p.operation(id).unwrap().common.depth_increment, 1.0);
    p.redo();
    assert_eq!(p.operation(id).unwrap().common.depth_increment, 3.0);
}

#[test]
fn new_parts_follow_machining_and_new_operations_go_into_the_current_part() {
    let (mut p, first) = with_pocket();
    let s = SharedSettings {
        plunge_feedrate: 222.0,
        ..Default::default()
    };
    p.set_machining_settings(s).unwrap();
    assert_eq!(p.new_part(), 1);
    let part = &p.document().parts[1];
    assert_eq!(part.name, "Part2");
    assert_eq!(part.settings.plunge_feedrate, 222.0);

    // New operations go into the current part; with none (or one that's gone), the first.
    let shape = p.add_shape("Default", circle(5.0)).unwrap();
    let in_first = p.add_profile(&[shape]).unwrap();
    p.set_current_part(Some("Part2".into()));
    let second = p.add_profile(&[shape]).unwrap();
    p.set_current_part(Some("Gone".into()));
    let third = p.add_profile(&[shape]).unwrap();
    let ids = |pi: usize| -> Vec<u32> {
        p.document().parts[pi]
            .operations
            .iter()
            .map(|o| o.id)
            .collect()
    };
    assert_eq!(ids(0), vec![first, in_first, third]);
    assert_eq!(ids(1), vec![second]);
}

#[test]
fn moving_an_operation_follows_the_new_part_and_keeps_own_values() {
    let (mut p, id) = with_pocket();
    let mut op = p.operation(id).unwrap().clone();
    op.common.cut_feedrate = 1500.0;
    p.set_operation(op).unwrap();
    p.new_part();
    let mut part = p.document().parts[1].settings.clone();
    part.cut_feedrate = 700.0;
    part.clearance_plane = 8.0;
    p.set_part_settings(1, part).unwrap();
    p.set_part_tool(1, Some(end_mill(3, 4.0))).unwrap();

    p.move_operation(id, 1).unwrap();
    assert!(p.document().parts[0].operations.is_empty());
    let op = p.operation(id).unwrap();
    assert_eq!(op.common.clearance_plane, 8.0, "followed from the new part");
    assert_eq!(op.common.cut_feedrate, 1500.0, "its own value");
    let state = p
        .operation_states()
        .into_iter()
        .find(|s| s.id == id)
        .unwrap();
    assert_eq!(state.tool_from, "part");
    assert_eq!(state.tool_number, Some(3));
    p.undo();
    assert_eq!(p.document().parts[0].operations[0].id, id);
}

#[test]
fn parts_are_renamed_with_unique_names_and_deleted_with_their_operations() {
    let (mut p, id) = with_pocket();
    p.new_part();
    assert!(p.rename_part(1, "  ").is_err());
    assert!(p.rename_part(1, "Part1").is_err());
    p.rename_part(1, " Lid ").unwrap();
    assert_eq!(p.document().parts[1].name, "Lid");
    assert_eq!(p.new_part(), 2);
    assert_eq!(p.document().parts[2].name, "Part2");

    p.delete_part(0).unwrap();
    assert!(p.operation(id).is_none());
    assert_eq!(p.document().parts.len(), 2);
    p.undo();
    assert!(p.operation(id).is_some());
    assert!(p.delete_part(9).is_err());
}

#[test]
fn project_files_keep_shared_settings_and_own_lists() {
    let (mut p, id) = with_pocket();
    p.set_machining_settings(changed()).unwrap();
    let mut part = p.document().parts[0].settings.clone();
    part.spindle_speed = 11000.0;
    p.set_part_settings(0, part).unwrap();
    let mut op = p.operation(id).unwrap().clone();
    op.common.cut_feedrate = 1500.0;
    p.set_operation(op).unwrap();

    let text = p.save_text("t");
    let mut q = Project::new();
    q.open(&text, "t").unwrap();
    assert_eq!(q.document(), p.document());
}

#[test]
fn project_files_in_the_older_format_are_refused() {
    let (mut p, _) = with_pocket();
    let text = p
        .save_text("t")
        .replace("\"format_version\": 4", "\"format_version\": 3");
    let err = Project::new().open(&text, "t").unwrap_err();
    assert!(err.contains("older WHAM_CAM"), "{err}");
}
