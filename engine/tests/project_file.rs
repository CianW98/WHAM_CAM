//! Project file tests: save and open round trips, bad files, unsaved-change tracking.

use wham_engine::project_file::{FORMAT_VERSION, from_text, to_text};
use wham_engine::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// A project with one of every shape type, odd numbers, two layers and a custom grid.
fn every_shape() -> Project {
    let mut proj = Project::new();
    let shapes = vec![
        Shape::Line {
            start: p(0.1, 1.0 / 3.0),
            end: p(1e-9, -123456.789),
        },
        Shape::Arc {
            center: p(2.0_f64.sqrt(), std::f64::consts::PI),
            radius: 0.30000000000000004,
            start_angle: 359.99999999,
            end_angle: 12.5,
        },
        Shape::Circle {
            center: p(-5.0, 5.0),
            radius: 7.25,
        },
        Shape::Polyline(Polyline {
            vertices: vec![
                Vertex {
                    x: 0.0,
                    y: 0.0,
                    bulge: 0.0,
                },
                Vertex {
                    x: 10.0,
                    y: 0.0,
                    bulge: -0.41421356237309503,
                },
                Vertex {
                    x: 10.0,
                    y: 10.0,
                    bulge: 0.0,
                },
            ],
            closed: true,
        }),
        Shape::rectangle(p(1.0, 2.0), 30.0, 20.0, 17.5),
        Shape::Points {
            points: vec![p(1.0, 1.0), p(2.5, -3.75)],
        },
        Shape::Spline(Spline {
            degree: 2,
            control_points: vec![p(0.0, 0.0), p(5.0, 10.0), p(10.0, 0.0)],
            knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            weights: vec![1.0, std::f64::consts::FRAC_1_SQRT_2, 1.0],
            closed: false,
        }),
        Shape::Ellipse {
            center: p(50.0, 50.0),
            major_axis: p(20.0, 5.0),
            ratio: 0.4,
            start_param: 0.0,
            end_param: std::f64::consts::TAU,
        },
    ];
    for (i, s) in shapes.into_iter().enumerate() {
        let layer = if i % 2 == 0 { "Default" } else { "Holes" };
        proj.add_shape(layer, s).unwrap();
    }
    proj.set_layer_color("Holes", "#00ffff").unwrap();
    proj.set_grid_spacing(2.5).unwrap();
    proj
}

#[test]
fn save_and_open_gives_back_the_same_project() {
    let mut proj = every_shape();
    let text = proj.save_text("bracket");
    let mut other = Project::new();
    other.open(&text, "bracket").unwrap();
    assert_eq!(other.document(), proj.document());
    assert_eq!(other.document().name, "bracket");
    assert_eq!(other.document().grid_spacing, 2.5);
    // And saving it again gives exactly the same text.
    assert_eq!(other.save_text("bracket"), text);
}

#[test]
fn file_is_labelled_with_format_and_version() {
    let text = to_text(Project::new().document());
    assert!(text.contains("\"format\": \"WHAM_CAM project\""));
    assert!(text.contains(&format!("\"format_version\": {FORMAT_VERSION}")));
    assert!(text.contains(&format!(
        "\"saved_by\": \"WHAM_CAM {}\"",
        env!("CARGO_PKG_VERSION")
    )));
}

#[test]
fn newer_format_is_refused_with_the_version() {
    let text = to_text(Project::new().document())
        .replace(
            &format!("\"format_version\": {FORMAT_VERSION}"),
            &format!("\"format_version\": {}", FORMAT_VERSION + 1),
        )
        .replace(
            &format!("WHAM_CAM {}", env!("CARGO_PKG_VERSION")),
            "WHAM_CAM 9.9.9",
        );
    let err = from_text(&text).unwrap_err();
    assert!(err.contains("newer") && err.contains("9.9.9"), "{err}");
}

#[test]
fn bad_files_are_refused_and_change_nothing() {
    let mut proj = every_shape();
    let before = proj.document().clone();
    let good = to_text(&before);
    let bad = [
        "this is not json".to_string(),
        "{\"layers\": []}".to_string(),
        good.replace("\"format\": \"WHAM_CAM project\",", ""),
        good.replacen("\"id\": 2", "\"id\": 1", 1),
        good.replace("\"Holes\"", "\"Default\""),
        good.replace("#00ffff", "blue"),
        good.replace("\"radius\": 7.25", "\"radius\": -1.0"),
        good.replace("\"grid_spacing\": 2.5", "\"grid_spacing\": 0.0"),
        good.replace("\"degree\": 2", "\"degree\": 3"),
    ];
    for text in &bad {
        assert!(proj.open(text, "x").is_err(), "accepted: {text}");
        assert_eq!(proj.document(), &before);
    }
}

#[test]
fn opening_starts_a_fresh_history() {
    let mut proj = every_shape();
    let text = proj.save_text("a");
    assert!(proj.can_undo());
    proj.open(&text, "a").unwrap();
    assert!(!proj.can_undo() && !proj.can_redo());
    assert!(!proj.is_modified());
}

#[test]
fn modified_tracking() {
    let mut proj = Project::new();
    assert!(!proj.is_modified());
    let id = proj
        .add_shape(
            "Default",
            Shape::Circle {
                center: p(0.0, 0.0),
                radius: 1.0,
            },
        )
        .unwrap();
    assert!(proj.is_modified());
    assert!(proj.undo());
    assert!(
        !proj.is_modified(),
        "undo back to the saved state is unchanged"
    );
    assert!(proj.redo());
    proj.save_text("one");
    proj.mark_saved();
    assert!(!proj.is_modified());
    proj.delete(&[id]);
    assert!(proj.is_modified());
    assert!(proj.undo());
    assert!(!proj.is_modified());
    // Undo after saving doesn't bring back the old project name.
    assert!(proj.undo());
    assert_eq!(proj.document().name, "one");
    proj.new_project();
    assert!(!proj.is_modified());
    assert!(proj.document().objects().next().is_none());
}
