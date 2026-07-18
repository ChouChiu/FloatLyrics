// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Frontend layer-shell positioning, snapping, and resize anchoring.

use gtk::prelude::*;
use gtk4_layer_shell::{Edge, LayerShell};
use std::{cell::RefCell, rc::Rc};

use crate::shared::config::WindowPosition;

mod geometry;

pub(super) use geometry::WindowPlacement;
use geometry::{
    DragOrigin, FloatingGeometry, bottom_margin_from_y, centered_position, dragged_placement,
    effective_surface_size, fallback_geometry, horizontal_position, placement_at, snap_css_classes,
    vertical_position, y_from_bottom_margin,
};

#[derive(Clone)]
pub(super) struct PlacementState(Rc<RefCell<WindowPlacement>>);

impl PlacementState {
    fn new(placement: WindowPlacement) -> Self {
        Self(Rc::new(RefCell::new(placement)))
    }

    pub(super) fn current(&self) -> WindowPlacement {
        *self.0.borrow()
    }

    fn set(&self, placement: WindowPlacement) {
        *self.0.borrow_mut() = placement;
    }
}

pub(super) fn attach_floating_drag(
    window: &gtk::ApplicationWindow,
    content: &gtk::Box,
    fallback_width: i32,
    fallback_height: i32,
    initial_placement: Option<WindowPlacement>,
    on_drag_end: impl Fn(WindowPosition) + 'static,
) -> PlacementState {
    let drag_origin = Rc::new(RefCell::new(DragOrigin::default()));
    let placement = PlacementState::new(initial_placement.unwrap_or_else(|| {
        placement_for_window(window, fallback_width, fallback_height).unwrap_or_default()
    }));
    let gesture = gtk::GestureDrag::new();

    {
        let window = window.clone();
        let drag_origin = Rc::clone(&drag_origin);
        let placement = placement.clone();
        gesture.connect_drag_begin(move |_, _, _| {
            let geometry = floating_geometry(&window, fallback_width, fallback_height)
                .unwrap_or_else(|| fallback_geometry(fallback_width, fallback_height));
            let bottom_margin = window.margin(Edge::Bottom);
            let origin = DragOrigin {
                x: window.margin(Edge::Left),
                y: y_from_bottom_margin(bottom_margin, geometry),
                geometry,
            };
            placement.set(placement_at(origin.x, origin.y, geometry));
            *drag_origin.borrow_mut() = origin;
        });
    }

    {
        let window = window.clone();
        let content = content.clone();
        let drag_origin = Rc::clone(&drag_origin);
        let placement = placement.clone();
        gesture.connect_drag_update(move |_, offset_x, offset_y| {
            let origin = *drag_origin.borrow();
            let (next_left, next_bottom, next_placement) =
                dragged_placement(origin, offset_x, offset_y);

            placement.set(next_placement);
            window.set_margin(Edge::Left, next_left);
            window.set_margin(Edge::Bottom, next_bottom);
            apply_snap_css_classes(&content, &next_placement);
        });
    }

    {
        let placement = placement.clone();
        gesture.connect_drag_end(move |_, _, _| {
            on_drag_end(placement.current().position());
        });
    }

    content.add_controller(gesture);
    placement
}

pub(super) fn initial_x(window_width: i32) -> Option<i32> {
    let monitor = first_monitor()?;
    Some(centered_position(monitor.geometry().width(), window_width))
}

pub(super) fn bottom_margin_from_placement(
    window: &gtk::ApplicationWindow,
    placement: &WindowPlacement,
    fallback_width: i32,
    fallback_height: i32,
) -> Option<i32> {
    let geometry = floating_geometry(window, fallback_width, fallback_height)?;
    let y = vertical_position(placement, geometry.viewport_height, geometry.surface_height);
    Some(bottom_margin_from_y(y, geometry))
}

pub(super) fn left_margin_for_width(
    window: &gtk::ApplicationWindow,
    placement: &WindowPlacement,
    window_width: i32,
) -> Option<i32> {
    let monitor = window_monitor(window).or_else(first_monitor)?;
    Some(horizontal_position(
        placement,
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

fn placement_for_window(
    window: &gtk::ApplicationWindow,
    fallback_width: i32,
    fallback_height: i32,
) -> Option<WindowPlacement> {
    let geometry = floating_geometry(window, fallback_width, fallback_height)?;
    Some(placement_at(
        window.margin(Edge::Left),
        y_from_bottom_margin(window.margin(Edge::Bottom), geometry),
        geometry,
    ))
}

fn floating_geometry(
    window: &gtk::ApplicationWindow,
    fallback_width: i32,
    fallback_height: i32,
) -> Option<FloatingGeometry> {
    let monitor = window_monitor(window).or_else(first_monitor)?;
    let geometry = monitor.geometry();
    let surface_width = effective_surface_size(window.width(), fallback_width);
    let surface_height = effective_surface_size(window.height(), fallback_height);

    Some(FloatingGeometry {
        viewport_width: geometry.width().max(0),
        viewport_height: geometry.height().max(0),
        surface_width,
        surface_height,
    })
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

const SNAP_CSS_CLASSES: &[&str] = &[
    "snapped-left",
    "snapped-right",
    "snapped-top",
    "snapped-bottom",
];

pub(super) fn apply_snap_css_classes(content: &gtk::Box, placement: &WindowPlacement) {
    let wanted = snap_css_classes(placement);
    for cls in SNAP_CSS_CLASSES {
        if wanted.contains(cls) {
            if !content.has_css_class(cls) {
                content.add_css_class(cls);
            }
        } else {
            content.remove_css_class(cls);
        }
    }
}
