//! Operations in a project: adding, editing, undo, generating, out-of-date tracking, saving.

use wham_engine::operation::{OperationKind, ProfileSide};
use wham_engine::*;

fn square(x0: f64, y0: f64, size: f64) -> Shape {
    Shape::rectangle(Point::new(x0, y0), size, size, 0.0)
}

fn project_with_pocket() -> (Project, u32, u32) {
    let mut proj = Project::new();
    let shape = proj.add_shape("Default", square(0.0, 0.0, 50.0)).unwrap();
    let op = proj.add_pocket(&[shape]).unwrap();
    (proj, shape, op)
}

fn status(proj: &Project, id: u32) -> String {
    proj.operation_states()
        .into_iter()
        .find(|s| s.id == id)
        .unwrap()
        .status
}

#[test]
fn add_pocket_names_it_and_undo_removes_it() {
    let (mut proj, shape, op) = project_with_pocket();
    let o = proj.operation(op).unwrap();
    assert_eq!(o.name, "Pocket1");
    assert_eq!(o.shape_ids, vec![shape]);
    assert!(matches!(o.kind, OperationKind::Pocket(_)));
    let second = proj.add_pocket(&[shape]).unwrap();
    assert_eq!(proj.operation(second).unwrap().name, "Pocket2");
    assert!(proj.undo());
    assert!(proj.operation(second).is_none());
    assert!(proj.undo());
    assert!(proj.operation(op).is_none());
}

#[test]
fn shapes_that_cant_be_pocketed_are_refused() {
    let mut proj = Project::new();
    let line = proj
        .add_shape(
            "Default",
            Shape::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(1.0, 1.0),
            },
        )
        .unwrap();
    assert!(proj.add_pocket(&[line]).is_err());
    assert!(proj.add_pocket(&[]).is_err());
    assert!(proj.add_pocket(&[999]).unwrap_err().contains("999"));
}

#[test]
fn generate_needs_a_target_depth_then_works() {
    let (mut proj, _, op) = project_with_pocket();
    assert_eq!(status(&proj, op), "none");
    let report = proj.generate(&[]);
    assert_eq!(report.generated, 0);
    assert!(
        report.errors[0].1.contains("target depth"),
        "{:?}",
        report.errors
    );
    assert_eq!(status(&proj, op), "error");

    let mut o = proj.operation(op).unwrap().clone();
    o.common.target_depth = Some(-3.0);
    proj.set_operation(o).unwrap();
    assert_eq!(status(&proj, op), "outdated");
    let report = proj.generate(&[]);
    assert_eq!(report.generated, 1, "{:?}", report.errors);
    assert_eq!(status(&proj, op), "ok");
    assert_eq!(proj.toolpaths_display().len(), 1);
}

#[test]
fn changes_mark_the_toolpath_out_of_date() {
    let (mut proj, shape, op) = project_with_pocket();
    let mut o = proj.operation(op).unwrap().clone();
    o.common.target_depth = Some(-2.0);
    proj.set_operation(o.clone()).unwrap();
    proj.generate(&[]);
    assert_eq!(status(&proj, op), "ok");

    // Moving the shape makes it out of date; undoing the move makes it current again.
    proj.transform(&[shape], TransformOp::Move { dx: 5.0, dy: 0.0 })
        .unwrap();
    assert_eq!(status(&proj, op), "outdated");
    assert!(proj.undo());
    assert_eq!(status(&proj, op), "ok");

    // So does a setting.
    o.common.tool_diameter = 3.0;
    proj.set_operation(o).unwrap();
    assert_eq!(status(&proj, op), "outdated");

    // A deleted shape gives an error on the next generate.
    proj.delete(&[shape]);
    let report = proj.generate(&[]);
    assert!(
        report.errors[0].1.contains("no longer exists"),
        "{:?}",
        report.errors
    );
}

#[test]
fn disabled_or_hidden_operations_are_not_drawn() {
    let (mut proj, _, op) = project_with_pocket();
    let mut o = proj.operation(op).unwrap().clone();
    o.common.target_depth = Some(-1.0);
    proj.set_operation(o.clone()).unwrap();
    proj.generate(&[]);
    assert_eq!(proj.toolpaths_display().len(), 1);
    o.show_toolpath = false;
    proj.set_operation(o.clone()).unwrap();
    assert!(proj.toolpaths_display().is_empty());
    o.show_toolpath = true;
    o.enabled = false;
    proj.set_operation(o).unwrap();
    assert!(proj.toolpaths_display().is_empty());
    // Generate (all) skips disabled operations.
    assert_eq!(proj.generate(&[]).generated, 0);
}

#[test]
fn operations_are_saved_and_toolpaths_are_not() {
    let (mut proj, _, op) = project_with_pocket();
    let mut o = proj.operation(op).unwrap().clone();
    o.common.target_depth = Some(-4.5);
    o.name = "Inner pocket".into();
    proj.set_operation(o.clone()).unwrap();
    proj.generate(&[]);
    let text = proj.save_text("t");
    let mut other = Project::new();
    other.open(&text, "t").unwrap();
    assert_eq!(other.operation(op), Some(&o));
    assert_eq!(status(&other, op), "none");
}

#[test]
fn set_operation_refuses_bad_values() {
    let (mut proj, _, op) = project_with_pocket();
    let mut o = proj.operation(op).unwrap().clone();
    o.common.cut_feedrate = f64::NAN;
    assert!(proj.set_operation(o.clone()).is_err());
    o.common.cut_feedrate = 800.0;
    o.name = "  ".into();
    assert!(proj.set_operation(o).is_err());
    assert!(proj.delete_operation(op));
    assert!(!proj.delete_operation(op));
}

#[test]
fn operation_shapes_can_be_changed_and_bad_ones_are_refused() {
    let (mut proj, shape, op) = project_with_pocket();
    let other = proj.add_shape("Default", square(100.0, 0.0, 30.0)).unwrap();
    proj.set_operation_shapes(op, &[shape, other]).unwrap();
    assert_eq!(proj.operation(op).unwrap().shape_ids, vec![shape, other]);
    // One undo step puts the old list back.
    assert!(proj.undo());
    assert_eq!(proj.operation(op).unwrap().shape_ids, vec![shape]);
    // A missing id or a shape that can't be pocketed changes nothing.
    let err = proj.set_operation_shapes(op, &[shape, 999]).unwrap_err();
    assert!(err.contains("999"), "{err}");
    let line = proj
        .add_shape(
            "Default",
            Shape::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(5.0, 0.0),
            },
        )
        .unwrap();
    assert!(proj.set_operation_shapes(op, &[line]).is_err());
    assert!(proj.set_operation_shapes(op, &[]).is_err());
    assert_eq!(proj.operation(op).unwrap().shape_ids, vec![shape]);
}

#[test]
fn add_profile_defaults_to_outside_or_on_line_for_open_shapes() {
    let mut proj = Project::new();
    let closed = proj.add_shape("Default", square(0.0, 0.0, 50.0)).unwrap();
    let line = proj
        .add_shape(
            "Default",
            Shape::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(10.0, 0.0),
            },
        )
        .unwrap();
    let a = proj.add_profile(&[closed]).unwrap();
    let b = proj.add_profile(&[closed, line]).unwrap();
    let side = |proj: &Project, id: u32| match &proj.operation(id).unwrap().kind {
        OperationKind::Profile(p) => p.side,
        _ => panic!("not a profile"),
    };
    assert_eq!(side(&proj, a), ProfileSide::Outside);
    assert_eq!(side(&proj, b), ProfileSide::OnLine);
    assert_eq!(proj.operation(a).unwrap().name, "Profile1");
    assert_eq!(proj.operation(b).unwrap().name, "Profile2");
    let open = |proj: &Project, id: u32| {
        proj.operation_states()
            .into_iter()
            .find(|s| s.id == id)
            .unwrap()
            .has_open_shapes
    };
    assert!(!open(&proj, a));
    assert!(open(&proj, b));

    // A line can be profiled but not pocketed; a pocket's shapes can't become open ones.
    let pocket = proj.add_pocket(&[closed]).unwrap();
    assert!(proj.set_operation_shapes(pocket, &[line]).is_err());
    assert!(proj.set_operation_shapes(a, &[line]).is_ok());

    // Undo removes the operations again, one step each.
    proj.undo();
    proj.undo();
    assert!(proj.operation(pocket).is_none());
    proj.undo();
    assert!(proj.operation(b).is_none());
}

#[test]
fn profile_generates_and_is_saved() {
    let mut proj = Project::new();
    let shape = proj.add_shape("Default", square(0.0, 0.0, 50.0)).unwrap();
    let op = proj.add_profile(&[shape]).unwrap();
    let mut o = proj.operation(op).unwrap().clone();
    o.common.target_depth = Some(-3.0);
    if let OperationKind::Profile(p) = &mut o.kind {
        p.overlap = 2.0;
        p.side = ProfileSide::Inside;
    }
    proj.set_operation(o.clone()).unwrap();
    let report = proj.generate(&[]);
    assert_eq!(report.generated, 1, "{:?}", report.errors);
    assert_eq!(status(&proj, op), "ok");

    let text = proj.save_text("t");
    let mut other = Project::new();
    other.open(&text, "t").unwrap();
    assert_eq!(other.operation(op), Some(&o));

    // An open shape with Outside is an error on generating, shown on the operation.
    let line = proj
        .add_shape(
            "Default",
            Shape::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(10.0, 0.0),
            },
        )
        .unwrap();
    proj.set_operation_shapes(op, &[line]).unwrap();
    let report = proj.generate(&[]);
    assert!(
        report.errors[0].1.contains("On line"),
        "{:?}",
        report.errors
    );
    assert_eq!(status(&proj, op), "error");
}
