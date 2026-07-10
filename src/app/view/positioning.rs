//! Layer-shell positioning, snapping, and resize anchoring.

use gtk::prelude::*;
use gtk4_layer_shell::{Edge, LayerShell};
use std::{cell::RefCell, rc::Rc};

const SNAP_THRESHOLD_PX: i32 = 12;

#[derive(Debug, Clone, Copy, PartialEq)]
enum AxisAnchor {
    Start,
    Center,
    End,
    Free(f64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct WindowPlacement {
    horizontal: AxisAnchor,
    vertical: AxisAnchor,
}

impl Default for WindowPlacement {
    fn default() -> Self {
        Self {
            horizontal: AxisAnchor::Center,
            vertical: AxisAnchor::Free(0.5),
        }
    }
}

pub(super) type SharedPlacement = Rc<RefCell<WindowPlacement>>;

#[derive(Debug, Clone, Copy, Default)]
struct DragOrigin {
    x: i32,
    y: i32,
    geometry: FloatingGeometry,
}

#[derive(Debug, Clone, Copy, Default)]
struct FloatingGeometry {
    viewport_width: i32,
    viewport_height: i32,
    surface_width: i32,
    surface_height: i32,
}

pub(super) fn attach_floating_drag(
    window: &gtk::ApplicationWindow,
    content: &gtk::Box,
    fallback_width: i32,
    fallback_height: i32,
    bottom_panel_height: i32,
) -> SharedPlacement {
    let drag_origin = Rc::new(RefCell::new(DragOrigin::default()));
    let placement = Rc::new(RefCell::new(WindowPlacement::default()));
    let gesture = gtk::GestureDrag::new();

    {
        let window = window.clone();
        let drag_origin = Rc::clone(&drag_origin);
        let placement = Rc::clone(&placement);
        gesture.connect_drag_begin(move |_, _, _| {
            let geometry =
                floating_geometry(&window, fallback_width, fallback_height, bottom_panel_height)
                    .unwrap_or_else(|| fallback_geometry(fallback_width, fallback_height));
            let bottom_margin = window.margin(Edge::Bottom);
            let origin = DragOrigin {
                x: window.margin(Edge::Left),
                y: y_from_bottom_margin(bottom_margin, geometry),
                geometry,
            };
            *placement.borrow_mut() = placement_at(origin.x, origin.y, geometry);
            *drag_origin.borrow_mut() = origin;
        });
    }

    {
        let window = window.clone();
        let drag_origin = Rc::clone(&drag_origin);
        let placement = Rc::clone(&placement);
        gesture.connect_drag_update(move |_, offset_x, offset_y| {
            let origin = *drag_origin.borrow();
            let (next_left, next_bottom, next_placement) =
                dragged_placement(origin, offset_x, offset_y);

            *placement.borrow_mut() = next_placement;
            window.set_margin(Edge::Left, next_left);
            window.set_margin(Edge::Bottom, next_bottom);
        });
    }

    content.add_controller(gesture);
    placement
}

pub(super) fn initial_x(window_width: i32) -> Option<i32> {
    let monitor = first_monitor()?;
    Some(position_for_anchor(
        AxisAnchor::Center,
        monitor.geometry().width(),
        window_width,
    ))
}

pub(super) fn left_margin_for_width(
    window: &gtk::ApplicationWindow,
    placement: &WindowPlacement,
    window_width: i32,
) -> Option<i32> {
    let monitor = window_monitor(window).or_else(first_monitor)?;
    Some(position_for_anchor(
        placement.horizontal,
        monitor.geometry().width(),
        window_width,
    ))
}

pub(super) fn available_panel_width(
    window: &gtk::ApplicationWindow,
    horizontal_gutter: i32,
) -> Option<i32> {
    let monitor = window_monitor(window).or_else(first_monitor)?;
    Some(
        monitor
            .geometry()
            .width()
            .saturating_sub(horizontal_gutter.saturating_mul(2))
            .max(0),
    )
}

fn dragged_placement(
    origin: DragOrigin,
    offset_x: f64,
    offset_y: f64,
) -> (i32, i32, WindowPlacement) {
    let max_x = maximum_position(
        origin.geometry.viewport_width,
        origin.geometry.surface_width,
    );
    let max_y = maximum_position(
        origin.geometry.viewport_height,
        origin.geometry.surface_height,
    );
    let raw_x = origin
        .x
        .saturating_add(offset_x.round() as i32)
        .clamp(0, max_x);
    let raw_y = origin
        .y
        .saturating_add(offset_y.round() as i32)
        .clamp(0, max_y);
    let (x, horizontal) = snap_axis(
        raw_x,
        origin.geometry.surface_width,
        origin.geometry.viewport_width,
    );
    let (y, vertical) = snap_axis(
        raw_y,
        origin.geometry.surface_height,
        origin.geometry.viewport_height,
    );

    (
        x,
        bottom_margin_from_y(y, origin.geometry),
        WindowPlacement {
            horizontal,
            vertical,
        },
    )
}

fn placement_at(x: i32, y: i32, geometry: FloatingGeometry) -> WindowPlacement {
    WindowPlacement {
        horizontal: snap_axis(x, geometry.surface_width, geometry.viewport_width).1,
        vertical: snap_axis(y, geometry.surface_height, geometry.viewport_height).1,
    }
}

fn snap_axis(position: i32, surface_size: i32, viewport_size: i32) -> (i32, AxisAnchor) {
    let maximum = maximum_position(viewport_size, surface_size);
    let position = position.clamp(0, maximum);
    let center = maximum / 2;

    if position <= SNAP_THRESHOLD_PX {
        (0, AxisAnchor::Start)
    } else if maximum.saturating_sub(position) <= SNAP_THRESHOLD_PX {
        (maximum, AxisAnchor::End)
    } else if position.abs_diff(center) <= SNAP_THRESHOLD_PX as u32 {
        (center, AxisAnchor::Center)
    } else {
        (
            position,
            AxisAnchor::Free(center_factor(position, surface_size, viewport_size)),
        )
    }
}

fn center_factor(position: i32, surface_size: i32, viewport_size: i32) -> f64 {
    if viewport_size <= 0 {
        return 0.5;
    }

    ((position as f64 + surface_size.max(0) as f64 / 2.0) / viewport_size as f64).clamp(0.0, 1.0)
}

fn position_for_anchor(anchor: AxisAnchor, viewport_size: i32, surface_size: i32) -> i32 {
    let maximum = maximum_position(viewport_size, surface_size);
    match anchor {
        AxisAnchor::Start => 0,
        AxisAnchor::Center => maximum / 2,
        AxisAnchor::End => maximum,
        AxisAnchor::Free(factor) => ((viewport_size.max(0) as f64 * factor.clamp(0.0, 1.0)
            - surface_size.max(0) as f64 / 2.0)
            .round() as i32)
            .clamp(0, maximum),
    }
}

fn maximum_position(viewport_size: i32, surface_size: i32) -> i32 {
    viewport_size.saturating_sub(surface_size).max(0)
}

fn floating_geometry(
    window: &gtk::ApplicationWindow,
    fallback_width: i32,
    fallback_height: i32,
    bottom_panel_height: i32,
) -> Option<FloatingGeometry> {
    let monitor = window_monitor(window).or_else(first_monitor)?;
    let geometry = monitor.geometry();
    let surface_width = effective_surface_size(window.width(), fallback_width);
    let surface_height = effective_surface_size(window.height(), fallback_height);

    Some(FloatingGeometry {
        viewport_width: geometry.width().max(0),
        viewport_height: geometry
            .height()
            .saturating_sub(bottom_panel_height)
            .max(0),
        surface_width,
        surface_height,
    })
}

fn fallback_geometry(fallback_width: i32, fallback_height: i32) -> FloatingGeometry {
    FloatingGeometry {
        viewport_width: fallback_width.max(0),
        viewport_height: fallback_height.max(0),
        surface_width: fallback_width.max(0),
        surface_height: fallback_height.max(0),
    }
}

fn effective_surface_size(actual: i32, fallback: i32) -> i32 {
    if actual > 0 { actual } else { fallback.max(0) }
}

fn y_from_bottom_margin(bottom_margin: i32, geometry: FloatingGeometry) -> i32 {
    (geometry.viewport_height - geometry.surface_height - bottom_margin).clamp(
        0,
        maximum_position(geometry.viewport_height, geometry.surface_height),
    )
}

fn bottom_margin_from_y(y: i32, geometry: FloatingGeometry) -> i32 {
    (geometry.viewport_height - geometry.surface_height - y).clamp(
        0,
        maximum_position(geometry.viewport_height, geometry.surface_height),
    )
}

fn window_monitor(window: &gtk::ApplicationWindow) -> Option<gtk::gdk::Monitor> {
    let display = gtk::gdk::Display::default()?;
    let surface = window.surface()?;
    display.monitor_at_surface(&surface)
}

fn first_monitor() -> Option<gtk::gdk::Monitor> {
    gtk::gdk::Display::default()?
        .monitors()
        .item(0)?
        .downcast::<gtk::gdk::Monitor>()
        .ok()
}

#[cfg(test)]
mod tests {
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
}
