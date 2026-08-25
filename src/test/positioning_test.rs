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

    let previous = placement_at(origin.x, origin.y, origin.geometry);
    let (left, bottom, _) = dragged_placement(origin, 25.4, -10.2, previous);
    assert_eq!((left, bottom), (125, 90));
    let (left, bottom, _) = dragged_placement(origin, -150.0, -500.0, previous);
    assert_eq!((left, bottom), (0, 500));
    let (left, bottom, _) = dragged_placement(origin, 500.0, 500.0, previous);
    assert_eq!((left, bottom), (500, 0));
}

#[test]
fn drag_snaps_to_edges_and_persists_the_edge_anchors() {
    let origin = DragOrigin {
        x: 100,
        y: 420,
        geometry: geometry(),
    };

    let previous = placement_at(origin.x, origin.y, origin.geometry);
    let (left, bottom, placement) = dragged_placement(origin, -92.0, 72.0, previous);
    assert_eq!((left, bottom), (0, 0));
    assert_eq!(
        snap_css_classes(&placement),
        vec!["snapped-left", "snapped-bottom"]
    );

    let restored = WindowPlacement::from_position(placement.position());
    assert_eq!(horizontal_position(&restored, 800, 300), 0);
    assert_eq!(vertical_position(&restored, 600, 100), 500);
}

#[test]
fn snaps_to_horizontal_edges_and_center() {
    assert_eq!(snap_axis(8, 300, 800), (0, AxisAnchor::Start));
    assert_eq!(snap_axis(245, 300, 800), (250, AxisAnchor::Center));
    assert_eq!(snap_axis(493, 300, 800), (500, AxisAnchor::End));
}

#[test]
fn center_axes_have_a_wider_capture_zone_and_release_hysteresis() {
    let origin = DragOrigin {
        x: 100,
        y: 100,
        geometry: geometry(),
    };
    let previous = placement_at(origin.x, origin.y, origin.geometry);

    let (left, bottom, centered) = dragged_placement(origin, 126.0, 126.0, previous);
    assert_eq!((left, bottom), (250, 250));

    let (left, bottom, still_centered) = dragged_placement(origin, 184.0, 184.0, centered);
    assert_eq!((left, bottom), (250, 250));

    let (left, bottom, released) = dragged_placement(origin, 199.0, 199.0, still_centered);
    assert_eq!((left, bottom), (299, 201));
    assert_eq!(snap_css_classes(&released), Vec::<&str>::new());
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
fn right_anchored_panel_stays_flush_during_width_changes() {
    let viewport_width = 1_920;

    for panel_width in [320, 640, 960] {
        let left = position_for_anchor(AxisAnchor::End, viewport_width, panel_width);
        assert_eq!(left + panel_width, viewport_width);
    }
}

#[test]
fn converts_between_top_y_and_bottom_margin() {
    assert_eq!(y_from_bottom_margin(0, geometry()), 500);
    assert_eq!(y_from_bottom_margin(500, geometry()), 0);
    assert_eq!(bottom_margin_from_y(0, geometry()), 500);
    assert_eq!(bottom_margin_from_y(500, geometry()), 0);
}

#[test]
fn window_position_round_trips_placement_anchors() {
    for placement in [
        WindowPlacement {
            horizontal: AxisAnchor::Start,
            vertical: AxisAnchor::End,
        },
        WindowPlacement {
            horizontal: AxisAnchor::Center,
            vertical: AxisAnchor::Free(0.72),
        },
    ] {
        assert_eq!(
            WindowPlacement::from_position(placement.position()),
            placement
        );
    }
}

#[test]
fn invalid_saved_position_falls_back_to_center() {
    assert_eq!(anchor_from_factor(f64::NAN), AxisAnchor::Center);
    assert_eq!(anchor_from_factor(-0.5), AxisAnchor::Start);
    assert_eq!(anchor_from_factor(1.5), AxisAnchor::End);
}
