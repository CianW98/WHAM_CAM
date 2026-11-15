use wham_engine::*;

const EPS: f64 = 1e-9;

#[test]
fn rectangle_has_four_correct_corners_and_is_closed() {
    let r = Polyline::rectangle(10.0, 20.0, 100.0, 80.0);
    assert!(r.closed);
    let corners: Vec<(f64, f64)> = r.vertices.iter().map(|v| (v.x, v.y)).collect();
    assert_eq!(
        corners,
        vec![(10.0, 20.0), (110.0, 20.0), (110.0, 100.0), (10.0, 100.0)]
    );
    assert!(r.vertices.iter().all(|v| v.bulge == 0.0));
}

#[test]
fn rectangle_display_points_are_its_corners() {
    let mut doc = Document::new("t");
    doc.layers[0].objects.push(DrawingObject {
        id: 7,
        shape: Shape::Polyline(Polyline::rectangle(0.0, 0.0, 100.0, 80.0)),
    });
    let data = display_data(&doc);
    assert_eq!(data.shapes.len(), 1);
    assert_eq!(data.shapes[0].id, 7);
    assert!(data.shapes[0].closed);
    assert_eq!(
        data.shapes[0].points,
        vec![[0.0, 0.0], [100.0, 0.0], [100.0, 80.0], [0.0, 80.0]]
    );
}

#[test]
fn circle_display_points_lie_on_the_radius() {
    let mut doc = Document::new("t");
    doc.layers[0].objects.push(DrawingObject {
        id: 1,
        shape: Shape::Circle {
            center: Point::new(50.0, 40.0),
            radius: 25.0,
        },
    });
    let shape = &display_data(&doc).shapes[0];
    assert!(shape.closed);
    assert!(shape.points.len() > 16);
    for p in &shape.points {
        let r = (p[0] - 50.0).hypot(p[1] - 40.0);
        assert!((r - 25.0).abs() < EPS, "point {p:?} is {r} from centre");
    }
    // Chord midpoints stay within the display tolerance of the true circle.
    for w in shape.points.windows(2) {
        let mid = [(w[0][0] + w[1][0]) / 2.0, (w[0][1] + w[1][1]) / 2.0];
        let r = (mid[0] - 50.0).hypot(mid[1] - 40.0);
        assert!(25.0 - r <= DISPLAY_TOLERANCE + EPS);
    }
}

#[test]
fn bulge_semicircle_is_drawn_on_the_correct_side() {
    // From (0,0) to (10,0) with bulge +1: counter-clockwise half circle, centre (5,0),
    // so it passes below the chord (through (5,-5)).
    let pl = Polyline {
        vertices: vec![
            Vertex {
                x: 0.0,
                y: 0.0,
                bulge: 1.0,
            },
            Vertex::line(10.0, 0.0),
        ],
        closed: false,
    };
    let mut doc = Document::new("t");
    doc.layers[0].objects.push(DrawingObject {
        id: 1,
        shape: Shape::Polyline(pl),
    });
    let pts = &display_data(&doc).shapes[0].points;
    assert_eq!(pts.first(), Some(&[0.0, 0.0]));
    assert_eq!(pts.last(), Some(&[10.0, 0.0]));
    for p in pts {
        let r = (p[0] - 5.0).hypot(p[1]);
        assert!((r - 5.0).abs() < 1e-6);
        assert!(p[1] <= 1e-9, "point {p:?} should be below the chord");
    }
}

#[test]
fn document_json_round_trips() {
    let doc = demo_document();
    let json = serde_json::to_string(&doc).unwrap();
    let back: Document = serde_json::from_str(&json).unwrap();
    assert_eq!(doc, back);
}
