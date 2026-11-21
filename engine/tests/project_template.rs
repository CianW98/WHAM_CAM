//! Project templates (spec 5.9) and the project's grid and snap settings.

use wham_engine::project_template::{DEFAULT_TEMPLATE, from_text, to_text};
use wham_engine::*;

fn area(x_min: f64, x_max: f64, y_min: f64, y_max: f64) -> GridArea {
    GridArea {
        x_min,
        x_max,
        y_min,
        y_max,
    }
}

fn circle() -> Shape {
    Shape::Circle {
        center: Point::new(0.0, 0.0),
        radius: 5.0,
    }
}

#[test]
fn starts_with_the_built_in_default_used_for_new_projects() {
    let t = ProjectTemplates::default();
    assert_eq!(t.templates.len(), 1);
    let d = t.for_new();
    assert_eq!(d.name, DEFAULT_TEMPLATE);
    assert_eq!(d.grid_spacing, DEFAULT_GRID_SPACING);
    assert_eq!(d.grid_area, area(-1000.0, 1000.0, -1000.0, 1000.0));
    assert_eq!(
        d.snap,
        SnapOptions {
            grid: true,
            objects: true
        }
    );
}

#[test]
fn add_rename_and_delete() {
    let mut t = ProjectTemplates::default();
    assert_eq!(t.add_template(), "Template 1");
    assert_eq!(t.add_template(), "Template 2");
    assert_eq!(
        t.rename_template("Template 1", "  Sheet  ").unwrap(),
        "Sheet"
    );
    assert!(
        t.rename_template("Sheet", "Template 2").is_err(),
        "names are unique"
    );
    assert!(t.rename_template("Sheet", " ").is_err(), "needs a name");
    assert!(t.rename_template("Nope", "X").is_err());
    t.delete_template("Template 2").unwrap();
    assert!(t.delete_template("Template 2").is_err());
    assert_eq!(
        t.templates
            .iter()
            .map(|x| x.name.as_str())
            .collect::<Vec<_>>(),
        ["Default", "Sheet"]
    );
}

#[test]
fn the_template_for_new_projects_cant_be_deleted_and_follows_a_rename() {
    let mut t = ProjectTemplates::default();
    let name = t.add_template();
    assert!(t.delete_template(DEFAULT_TEMPLATE).is_err());
    t.set_for_new_projects(&name).unwrap();
    t.delete_template(DEFAULT_TEMPLATE).unwrap();
    t.rename_template(&name, "Router").unwrap();
    assert_eq!(t.for_new_projects, "Router");
    assert_eq!(t.for_new().name, "Router");
    assert!(t.set_for_new_projects("Nope").is_err());
}

#[test]
fn values_are_checked() {
    let mut t = ProjectTemplates::default();
    assert!(t.set_grid_spacing("Default", 0.0).is_err());
    assert!(t.set_grid_spacing("Default", f64::NAN).is_err());
    t.set_grid_spacing("Default", 5.0).unwrap();
    assert!(
        t.set_grid_area("Default", area(10.0, 10.0, 0.0, 5.0))
            .is_err()
    );
    assert!(
        t.set_grid_area("Default", area(0.0, 5.0, 3.0, 1.0))
            .is_err()
    );
    assert!(
        t.set_grid_area("Default", area(0.0, f64::INFINITY, 0.0, 1.0))
            .is_err()
    );
    t.set_grid_area("Default", area(0.0, 1220.0, 0.0, 2440.0))
        .unwrap();
    t.set_snap(
        "Default",
        SnapOptions {
            grid: false,
            objects: true,
        },
    )
    .unwrap();
    let d = t.for_new();
    assert_eq!(d.grid_spacing, 5.0);
    assert_eq!(d.grid_area, area(0.0, 1220.0, 0.0, 2440.0));
    assert!(!d.snap.grid && d.snap.objects);
}

#[test]
fn file_text_round_trips() {
    let mut t = ProjectTemplates::default();
    let name = t.add_template();
    t.set_grid_spacing(&name, 0.1).unwrap();
    t.set_for_new_projects(&name).unwrap();
    let text = to_text(&t);
    assert!(text.contains("\"format\": \"WHAM_CAM project templates\""));
    assert_eq!(from_text(&text).unwrap(), t);
}

#[test]
fn damaged_files_give_messages() {
    assert!(from_text("not json").is_err());
    assert!(from_text(r#"{"format":"WHAM_CAM machines","format_version":1}"#).is_err());
    let good = to_text(&ProjectTemplates::default());
    let newer = good.replace("\"format_version\": 1", "\"format_version\": 99");
    assert!(from_text(&newer).unwrap_err().contains("newer"));
    let bad_spacing = good.replace("\"grid_spacing\": 10.0", "\"grid_spacing\": 0.0");
    assert!(from_text(&bad_spacing).unwrap_err().contains("damaged"));
    let bad_default = good.replace(
        "\"for_new_projects\": \"Default\"",
        "\"for_new_projects\": \"Gone\"",
    );
    assert!(from_text(&bad_default).unwrap_err().contains("Gone"));
}

#[test]
fn applying_a_template_copies_its_values_and_isnt_an_undo_step() {
    let mut t = ProjectTemplates::default();
    let name = t.add_template();
    t.set_grid_spacing(&name, 2.0).unwrap();
    t.set_grid_area(&name, area(0.0, 600.0, 0.0, 400.0))
        .unwrap();
    t.set_snap(
        &name,
        SnapOptions {
            grid: false,
            objects: false,
        },
    )
    .unwrap();

    let mut p = Project::new();
    p.add_shape("Default", circle()).unwrap();
    p.apply_template(t.template(&name).unwrap());
    let d = p.document();
    assert_eq!(d.template.as_deref(), Some("Template 1"));
    assert_eq!(d.grid_spacing, 2.0);
    assert_eq!(d.grid_area, area(0.0, 600.0, 0.0, 400.0));
    assert!(!d.snap.grid && !d.snap.objects);
    assert!(p.is_modified());

    // Undo takes back the circle but not the settings.
    assert!(p.undo());
    assert!(p.document().objects().next().is_none());
    assert_eq!(p.document().grid_spacing, 2.0);
    assert!(p.redo());
    assert_eq!(p.document().grid_spacing, 2.0);

    // Changing the template afterwards doesn't change the project.
    t.set_grid_spacing(&name, 7.0).unwrap();
    assert_eq!(p.document().grid_spacing, 2.0);
}

#[test]
fn project_settings_are_checked_and_not_undo_steps() {
    let mut p = Project::new();
    assert!(p.set_grid_area(area(5.0, 1.0, 0.0, 1.0)).is_err());
    p.set_grid_area(area(0.0, 100.0, 0.0, 50.0)).unwrap();
    p.set_snap(SnapOptions {
        grid: true,
        objects: false,
    });
    assert!(p.is_modified());
    assert!(!p.undo(), "nothing to undo");
    assert_eq!(p.document().grid_area, area(0.0, 100.0, 0.0, 50.0));
    assert!(!p.document().snap.objects);
}

#[test]
fn new_projects_start_from_the_template() {
    let mut t = ProjectTemplates::default();
    let name = t.add_template();
    t.set_grid_spacing(&name, 25.0).unwrap();
    t.set_for_new_projects(&name).unwrap();
    let mut p = Project::new();
    p.add_shape("Default", circle()).unwrap();
    p.new_project(t.for_new());
    assert_eq!(p.document().grid_spacing, 25.0);
    assert_eq!(p.document().template.as_deref(), Some("Template 1"));
    assert!(!p.is_modified());
    assert!(p.document().objects().next().is_none());
}

#[test]
fn grid_and_snap_settings_are_saved_in_the_project_file() {
    let mut p = Project::new();
    let mut t = ProjectTemplates::default();
    t.set_grid_area("Default", area(0.0, 1220.0, 0.0, 2440.0))
        .unwrap();
    p.apply_template(t.for_new());
    p.set_snap(SnapOptions {
        grid: false,
        objects: true,
    });
    let text = p.save_text("job");
    let doc = project_file::from_text(&text).unwrap();
    assert_eq!(doc.grid_area, area(0.0, 1220.0, 0.0, 2440.0));
    assert!(!doc.snap.grid && doc.snap.objects);
    assert_eq!(doc.template.as_deref(), Some("Default"));

    // A damaged grid area is refused with a message.
    let bad = text.replace("\"x_max\": 1220.0", "\"x_max\": -5.0");
    assert!(project_file::from_text(&bad).unwrap_err().contains("X min"));
}
