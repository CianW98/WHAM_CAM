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
