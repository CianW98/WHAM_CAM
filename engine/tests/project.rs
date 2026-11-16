//! Project tests: import, undo/redo, delete and selection hit-testing.

use wham_engine::*;

fn sample_dxf(insunits: i32) -> String {
    format!(
        "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n{insunits}\n0\nENDSEC\n\
         0\nSECTION\n2\nENTITIES\n\
         0\nLINE\n8\nCUT\n10\n0\n20\n0\n11\n10\n21\n0\n\
         0\nCIRCLE\n8\nCUT\n10\n20\n20\n0\n40\n1\n\
         0\nTEXT\n10\n0\n20\n0\n1\nlabel\n\
         0\nENDSEC\n0\nEOF\n"
    )
}

#[test]
fn starts_empty() {
    let p = Project::new();
    assert_eq!(p.document().objects().count(), 0);
    assert_eq!(p.display().bounds, None);
    assert!(!p.can_undo());
}

#[test]
fn dxf_info_reports_units_without_importing() {
    let info = Project::dxf_info(&sample_dxf(1)).unwrap();
    assert_eq!(info.units.name, "inches");
    assert_eq!(info.shape_count, 2);
}

#[test]
fn import_adds_objects_with_unique_ids_and_reports_skips() {
    let mut p = Project::new();
    let r = p.import_dxf(&sample_dxf(4), 1.0).unwrap();
    assert_eq!(r.objects_added, 2);
    assert_eq!(r.layers, vec!["CUT".to_string()]);
    assert_eq!(r.skipped.get("TEXT"), Some(&1));
    p.import_dxf(&sample_dxf(4), 1.0).unwrap();
    let ids: Vec<u32> = p.document().objects().map(|o| o.id).collect();
    assert_eq!(ids, vec![1, 2, 3, 4]);
    // "Default" layer stays, "CUT" is added once.
    let names: Vec<&str> = p
        .document()
        .layers
        .iter()
        .map(|l| l.name.as_str())
        .collect();
    assert_eq!(names, vec!["Default", "CUT"]);
    // Bounds come from the on-screen approximation: within the display tolerance of exact.
    let b = p.display().bounds.unwrap();
    for (got, want) in b.iter().zip([0.0, -1.0, 21.0, 1.0]) {
        assert!((got - want).abs() <= DISPLAY_TOLERANCE, "{b:?}");
    }
}

#[test]
fn import_scale_converts_inches_to_mm() {
    let mut p = Project::new();
    p.import_dxf(&sample_dxf(1), 25.4).unwrap();
    let shapes: Vec<&Shape> = p.document().objects().map(|o| &o.shape).collect();
    assert_eq!(
        shapes[0],
        &Shape::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(254.0, 0.0)
        }
    );
    assert_eq!(
        shapes[1],
        &Shape::Circle {
            center: Point::new(508.0, 0.0),
            radius: 25.4
        }
    );
}

#[test]
fn a_failed_import_changes_nothing() {
    let mut p = Project::new();
    assert!(p.import_dxf("not a dxf", 1.0).is_err());
    assert!(!p.can_undo());
}

#[test]
fn undo_and_redo_import_and_delete() {
    let mut p = Project::new();
    p.import_dxf(&sample_dxf(4), 1.0).unwrap();
    assert_eq!(p.delete(&[1]), 1);
    assert_eq!(p.document().objects().count(), 1);

    assert!(p.undo()); // undo delete
    assert_eq!(p.document().objects().count(), 2);
    assert!(p.undo()); // undo import
    assert_eq!(p.document().objects().count(), 0);
    assert!(!p.undo());

    assert!(p.redo());
    assert!(p.redo());
    assert_eq!(p.document().objects().count(), 1);
    assert!(!p.redo());

    // A new change clears redo.
    p.undo();
    p.delete(&[2]);
    assert!(!p.can_redo());
}

#[test]
fn deleting_nothing_is_not_an_undo_step() {
    let mut p = Project::new();
    p.import_dxf(&sample_dxf(4), 1.0).unwrap();
    assert_eq!(p.delete(&[99]), 0);
    p.undo();
    assert_eq!(
        p.document().objects().count(),
        0,
        "the undo should have undone the import"
    );
}

#[test]
fn pick_finds_the_nearest_shape_within_tolerance() {
    let mut p = Project::new();
    p.import_dxf(&sample_dxf(4), 1.0).unwrap();
    // Line is id 1 from (0,0) to (10,0); circle is id 2 at (20,0) r 1.
    assert_eq!(p.pick(5.0, 0.3, 0.5), Some(1));
    assert_eq!(p.pick(5.0, 0.6, 0.5), None);
    assert_eq!(p.pick(20.0, 1.2, 0.5), Some(2));
    // Inside the circle but far from its edge: not picked.
    assert_eq!(p.pick(20.0, 0.0, 0.5), None);
}

#[test]
fn box_selection_window_and_crossing() {
    let mut p = Project::new();
    p.import_dxf(&sample_dxf(4), 1.0).unwrap();
    // Box around the line only.
    assert_eq!(p.pick_box(-1.0, -1.0, 11.0, 1.0, BoxMode::Window), vec![1]);
    // Box cutting through the middle of the line, touching nothing else.
    assert_eq!(
        p.pick_box(4.0, -1.0, 6.0, 1.0, BoxMode::Window),
        Vec::<u32>::new()
    );
    assert_eq!(p.pick_box(4.0, -1.0, 6.0, 1.0, BoxMode::Crossing), vec![1]);
    // Box inside the circle, not touching its edge.
    assert_eq!(
        p.pick_box(19.5, -0.5, 20.5, 0.5, BoxMode::Crossing),
        Vec::<u32>::new()
    );
    // Everything.
    assert_eq!(
        p.pick_box(-5.0, -5.0, 25.0, 5.0, BoxMode::Window),
        vec![1, 2]
    );
}

fn layer_names(p: &Project) -> Vec<&str> {
    p.document()
        .layers
        .iter()
        .map(|l| l.name.as_str())
        .collect()
}

#[test]
fn deleting_all_objects_keeps_the_empty_layer() {
    let mut p = Project::new();
    p.import_dxf(&sample_dxf(4), 1.0).unwrap();
    p.delete(&[1, 2]);
    assert_eq!(layer_names(&p), vec!["Default", "CUT"]);
    assert!(p.document().layers[1].objects.is_empty());
}

#[test]
fn delete_layer_removes_it_and_its_objects_and_undo_restores_them() {
    let mut p = Project::new();
    p.import_dxf(&sample_dxf(4), 1.0).unwrap();
    assert!(p.delete_layer("CUT"));
    assert_eq!(layer_names(&p), vec!["Default"]);
    assert_eq!(p.document().objects().count(), 0);
    assert!(p.undo());
    assert_eq!(layer_names(&p), vec!["Default", "CUT"]);
    assert_eq!(p.document().objects().count(), 2);
    assert!(!p.delete_layer("NOPE"));
}

#[test]
fn deleting_the_last_layer_leaves_an_empty_default_layer() {
    let mut p = Project::new();
    p.import_dxf(&sample_dxf(4), 1.0).unwrap();
    p.delete_layer("Default");
    p.delete_layer("CUT");
    assert_eq!(layer_names(&p), vec!["Default"]);
    assert_eq!(p.document().objects().count(), 0);
    // Undo goes back one step at a time.
    p.undo();
    assert_eq!(layer_names(&p), vec!["CUT"]);
}

#[test]
fn rename_layer_checks_the_name_and_is_undoable() {
    let mut p = Project::new();
    p.import_dxf(&sample_dxf(4), 1.0).unwrap();
    assert_eq!(p.rename_layer("CUT", "  Outline  "), Ok(()));
    assert_eq!(layer_names(&p), vec!["Default", "Outline"]);
    assert!(p.rename_layer("Outline", "").is_err());
    assert!(p.rename_layer("Outline", "Default").is_err());
    assert!(p.rename_layer("Missing", "X").is_err());
    // Renaming to the same name is fine and is not an undo step.
    assert_eq!(p.rename_layer("Outline", "Outline"), Ok(()));
    p.undo();
    assert_eq!(layer_names(&p), vec!["Default", "CUT"]);
}

#[test]
fn layer_colour_is_used_for_its_shapes_and_is_undoable() {
    let mut p = Project::new();
    p.import_dxf(&sample_dxf(4), 1.0).unwrap();
    // New layers get a different palette colour from existing ones.
    let colors: Vec<&str> = p
        .document()
        .layers
        .iter()
        .map(|l| l.color.as_str())
        .collect();
    assert_eq!(colors, vec![LAYER_PALETTE[0], LAYER_PALETTE[1]]);
    assert!(
        p.display()
            .shapes
            .iter()
            .all(|s| s.color == LAYER_PALETTE[1])
    );

    assert_eq!(p.set_layer_color("CUT", "#00FF80"), Ok(()));
    assert!(p.display().shapes.iter().all(|s| s.color == "#00ff80"));
    assert!(p.set_layer_color("CUT", "green").is_err());
    assert!(p.set_layer_color("CUT", "#12345").is_err());
    p.undo();
    assert!(
        p.display()
            .shapes
            .iter()
            .all(|s| s.color == LAYER_PALETTE[1])
    );
}

#[test]
fn documents_saved_without_layer_colours_still_load() {
    let json = r#"{"name":"old","layers":[{"name":"L","objects":[]}],"parts":[]}"#;
    let doc: Document = serde_json::from_str(json).unwrap();
    assert_eq!(doc.layers[0].color, LAYER_PALETTE[0]);
}

#[test]
fn add_layer_picks_a_free_name_and_colour() {
    let mut proj = Project::new();
    assert_eq!(proj.add_layer(), "Layer 1");
    proj.rename_layer("Layer 1", "Keep").unwrap();
    assert_eq!(proj.add_layer(), "Layer 1");
    assert_eq!(proj.add_layer(), "Layer 2");
    let layers = &proj.document().layers;
    let names: Vec<&str> = layers.iter().map(|l| l.name.as_str()).collect();
    assert!(
        names.ends_with(&["Keep", "Layer 1", "Layer 2"]),
        "{names:?}"
    );
    let colours: std::collections::HashSet<&str> =
        layers.iter().map(|l| l.color.as_str()).collect();
    assert_eq!(colours.len(), layers.len());
    let before = layers.len();
    assert!(proj.undo());
    assert_eq!(proj.document().layers.len(), before - 1);
}
