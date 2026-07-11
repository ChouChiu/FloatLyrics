use super::*;

fn geometry() -> FloatingGeometry {
    FloatingGeometry {
        viewport_width: 800,
        viewport_height: 600,
        surface_width: 300,
        surface_height: 100,
    }
}

#[test]
fn free_drag_stays_inside_viewport() {
    let origin = DragOrigin {
        x: 100,
        y: 420,
        geometry: geometry(),
    };

    let (left, bottom, _) = dragged_placement(origin, 25.4, -10.2);
    assert_eq!((left, bottom), (125, 90));
    let (left, bottom, _) = dragged_placement(origin, -150.0, -500.0);
    assert_eq!((left, bottom), (0, 500));
    let (left, bottom, _) = dragged_placement(origin, 500.0, 500.0);
    assert_eq!((left, bottom), (500, 0));
}

#[test]
fn snaps_to_horizontal_edges_and_center() {
    assert_eq!(snap_axis(8, 300, 800), (0, AxisAnchor::Start));
    assert_eq!(snap_axis(245, 300, 800), (250, AxisAnchor::Center));
    assert_eq!(snap_axis(493, 300, 800), (500, AxisAnchor::End));
}

#[test]
fn snaps_to_vertical_edges_and_center() {
    assert_eq!(snap_axis(10, 100, 600), (0, AxisAnchor::Start));
    assert_eq!(snap_axis(258, 100, 600), (250, AxisAnchor::Center));
    assert_eq!(snap_axis(492, 100, 600), (500, AxisAnchor::End));
}

#[test]
fn anchored_resize_uses_expected_expansion_direction() {
    assert_eq!(position_for_anchor(AxisAnchor::Start, 1_200, 700), 0);
    assert_eq!(position_for_anchor(AxisAnchor::Center, 1_200, 700), 250);
    assert_eq!(position_for_anchor(AxisAnchor::End, 1_200, 700), 500);
    assert_eq!(position_for_anchor(AxisAnchor::Free(0.6), 1_200, 700), 370);
}

#[test]
fn converts_between_top_y_and_bottom_margin() {
    assert_eq!(y_from_bottom_margin(0, geometry()), 500);
    assert_eq!(y_from_bottom_margin(500, geometry()), 0);
    assert_eq!(bottom_margin_from_y(0, geometry()), 500);
    assert_eq!(bottom_margin_from_y(500, geometry()), 0);
}
