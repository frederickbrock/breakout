use super::*;

#[test]
fn a_rounded_rect_spans_its_size_and_is_convex() {
    let size = Vec2::new(150.0, 100.0);
    let vertices = rounded_rect_vertices(size, PLATE_RADIUS);
    let min = vertices.iter().copied().reduce(Vec2::min).unwrap();
    let max = vertices.iter().copied().reduce(Vec2::max).unwrap();
    assert!((max - size / 2.0).abs().max_element() < 1e-3);
    assert!((min + size / 2.0).abs().max_element() < 1e-3);
    // The corners are cut: no vertex sits on the bounding box's corner.
    let on_box_corner = |v: &Vec2| (v.abs() - size / 2.0).abs().max_element() < 1e-3;
    assert!(!vertices.iter().any(on_box_corner));
    assert!(ConvexPolygon::new(vertices).is_ok());
}

#[test]
fn a_plate_gets_a_mesh() {
    let mut app = crate::test_support::app();
    app.update();
    let plate = app
        .world_mut()
        .spawn(BackingPlate {
            size: Vec2::new(100.0, 40.0),
        })
        .id();
    app.update();
    assert!(app.world().get::<Mesh2d>(plate).is_some());
    assert!(app
        .world()
        .get::<MeshMaterial2d<ColorMaterial>>(plate)
        .is_some());
}
