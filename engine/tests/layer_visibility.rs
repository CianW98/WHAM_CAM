//! Showing and hiding layers (decided 2026-11-20): a view setting only. Hidden layers aren't
//! drawn, picked, box-selected, snapped to or selected by Select All; nothing is saved or undone.

use wham_engine::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// A project with a line on "Default" and a circle on "Holes".
fn project() -> (Project, u32, u32) {
    let mut proj = Project::new();
    let line = proj
        .add_shape(
            "Default",
            Shape::Line {
                start: p(0.0, 0.0),
                end: p(10.0, 0.0),
            },
        )
        .unwrap();
    let circle = proj
        .add_shape(
            "Holes",
            Shape::Circle {
                center: p(50.0, 0.0),
                radius: 5.0,
            },
        )
        .unwrap();
    (proj, line, circle)
}

const SNAPS: SnapOptions = SnapOptions {
    grid: false,
    objects: true,
};

#[test]
fn a_hidden_layer_is_not_drawn_picked_snapped_or_selected() {
    let (mut proj, line, circle) = project();
    proj.set_layer_visible("Holes", false).unwrap();
    assert_eq!(proj.hidden_layers(), ["Holes"]);
    let drawn: Vec<u32> = proj.display().shapes.iter().map(|s| s.id).collect();
    assert_eq!(drawn, [line]);
    assert_eq!(proj.pick(55.0, 0.0, 0.5), None);
    assert!(
        proj.pick_box(40.0, -10.0, 60.0, 10.0, BoxMode::Crossing)
            .is_empty()
    );
    assert_eq!(proj.snap(50.0, 0.2, 0.5, SNAPS, &[]).kind, SnapKind::None);
    assert_eq!(proj.all_ids(), [line]);
    // Shown again: all back.
    proj.set_layer_visible("Holes", true).unwrap();
    assert_eq!(proj.pick(55.0, 0.0, 0.5), Some(circle));
    assert_eq!(proj.all_ids(), [line, circle]);
}

#[test]
fn hiding_is_not_saved_undone_or_a_change() {
    let (mut proj, _, _) = project();
    let saved = proj.save_text("x");
    proj.mark_saved();
    proj.set_layer_visible("Holes", false).unwrap();
    assert!(!proj.is_modified());
    assert_eq!(proj.save_text("x"), saved);
    // Undo undoes the last real edit (the circle), not the hiding.
    assert!(proj.undo());
    assert_eq!(proj.document().objects().count(), 1);
    // Opening a project shows every layer.
    proj.open(&saved, "x").unwrap();
    assert!(proj.hidden_layers().is_empty());
}

#[test]
fn new_objects_on_a_hidden_layer_show_it_again() {
    let (mut proj, _, circle) = project();
    proj.copy(&[circle]);
    proj.set_layer_visible("Holes", false).unwrap();
    proj.paste(p(80.0, 0.0));
    assert!(proj.hidden_layers().is_empty());
}

#[test]
fn renaming_keeps_a_layer_hidden_and_unknown_layers_are_an_error() {
    let (mut proj, _, _) = project();
    proj.set_layer_visible("Holes", false).unwrap();
    proj.rename_layer("Holes", "Drill holes").unwrap();
    assert_eq!(proj.hidden_layers(), ["Drill holes"]);
    assert!(proj.set_layer_visible("Nope", false).is_err());
    proj.delete_layer("Drill holes");
    assert!(proj.hidden_layers().is_empty());
}

#[test]
fn operations_still_use_shapes_on_hidden_layers() {
    let (mut proj, _, circle) = project();
    proj.set_layer_visible("Holes", false).unwrap();
    assert!(proj.add_drill(&[circle]).is_ok());
}
