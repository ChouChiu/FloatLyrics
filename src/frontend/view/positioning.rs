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
    DragOrigin, FloatingGeometry, bottom_margin_from_y, centered_position, dragged_free_placement,
    dragged_placement, effective_surface_size, fallback_geometry, horizontal_position,
    placement_at, snap_css_classes, vertical_position, y_from_bottom_margin,
};

#[derive(Clone)]
pub(super) struct PlacementState {
    content: Rc<RefCell<PlacedContent>>,
    mode: DragMode,
}

#[derive(Clone, Copy)]
struct PlacedContent {
    placement: WindowPlacement,
    left: i32,
    top: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DragMode {
    LayerSurface,
    InternalPanel,
}

impl DragMode {
    pub(super) fn is_internal(self) -> bool {
        self == Self::InternalPanel
    }
}

impl PlacementState {
    fn new(placement: WindowPlacement, left: i32, top: i32, mode: DragMode) -> Self {
        Self {
            content: Rc::new(RefCell::new(PlacedContent {
                placement,
                left,
                top,
            })),
            mode,
        }
    }

    pub(super) fn current(&self) -> WindowPlacement {
        self.content.borrow().placement
    }

    fn position(&self) -> (i32, i32) {
        let content = *self.content.borrow();
        (content.left, content.top)
    }

    pub(super) fn uses_internal_drag(&self) -> bool {
        self.mode.is_internal()
    }

    fn set(&self, placement: WindowPlacement, left: i32, top: i32) {
        *self.content.borrow_mut() = PlacedContent {
            placement,
            left,
            top,
        };
    }
}

pub(super) struct FloatingDragLayout {
    pub(super) stage: gtk::Fixed,
    pub(super) fallback_width: i32,
    pub(super) fallback_height: i32,
    pub(super) initial_placement: Option<WindowPlacement>,
    pub(super) initial_bottom_margin: i32,
    pub(super) mode: DragMode,
}

pub(super) fn attach_floating_drag(
    window: &gtk::ApplicationWindow,
    content: &gtk::Box,
    drag_handle: &gtk::Box,
    layout: FloatingDragLayout,
    on_drag_end: impl Fn(WindowPosition) + 'static,
) -> PlacementState {
    let FloatingDragLayout {
        stage,
        fallback_width,
        fallback_height,
        initial_placement,
        initial_bottom_margin,
        mode,
    } = layout;
    let drag_origin = Rc::new(RefCell::new(DragOrigin::default()));
    let geometry = floating_geometry(window, content, mode, fallback_width, fallback_height)
        .unwrap_or_else(|| fallback_geometry(fallback_width, fallback_height));
    let initial_placement = initial_placement.unwrap_or_else(|| {
        if mode.is_internal() {
            let left = centered_position(geometry.viewport_width, geometry.surface_width);
            let top = y_from_bottom_margin(initial_bottom_margin, geometry);
            placement_at(left, top, geometry)
        } else {
            placement_for_window(window, geometry)
        }
    });
    let initial_left = horizontal_position(
        &initial_placement,
        geometry.viewport_width,
        geometry.surface_width,
    );
    let initial_top = vertical_position(
        &initial_placement,
        geometry.viewport_height,
        geometry.surface_height,
    );
    if mode.is_internal() {
        stage.put(content, initial_left as f64, initial_top as f64);
    } else {
        stage.put(content, 0.0, 0.0);
    }
    let placement = PlacementState::new(initial_placement, initial_left, initial_top, mode);
    let gesture = gtk::GestureDrag::new();
    drag_handle.set_cursor_from_name(Some("grab"));

    {
        let window = window.downgrade();
        let stage = stage.downgrade();
        let content = content.downgrade();
        let drag_handle = drag_handle.downgrade();
        let drag_origin = Rc::clone(&drag_origin);
        let placement = placement.clone();
        gesture.connect_drag_begin(move |_, _, _| {
            let (Some(window), Some(stage), Some(content), Some(drag_handle)) = (
                window.upgrade(),
                stage.upgrade(),
                content.upgrade(),
                drag_handle.upgrade(),
            ) else {
                return;
            };
            drag_handle.set_cursor_from_name(Some("grabbing"));
            let geometry =
                floating_geometry(&window, &content, mode, fallback_width, fallback_height)
                    .unwrap_or_else(|| fallback_geometry(fallback_width, fallback_height));
            let (x, y) = if mode.is_internal() {
                let current = placement.current();
                (
                    horizontal_position(&current, geometry.viewport_width, geometry.surface_width),
                    vertical_position(&current, geometry.viewport_height, geometry.surface_height),
                )
            } else {
                (
                    window.margin(Edge::Left),
                    y_from_bottom_margin(window.margin(Edge::Bottom), geometry),
                )
            };
            let origin = DragOrigin { x, y, geometry };
            if mode.is_internal() {
                if placement.position() != (x, y) {
                    stage.move_(&content, x as f64, y as f64);
                }
                placement.set(placement.current(), x, y);
            } else {
                placement.set(placement_at(x, y, geometry), x, y);
            }
            *drag_origin.borrow_mut() = origin;
        });
    }

    {
        let window = window.downgrade();
        let stage = stage.downgrade();
        let content = content.downgrade();
        let drag_origin = Rc::clone(&drag_origin);
        let placement = placement.clone();
        gesture.connect_drag_update(move |_, offset_x, offset_y| {
            let (Some(window), Some(stage), Some(content)) =
                (window.upgrade(), stage.upgrade(), content.upgrade())
            else {
                return;
            };
            let origin = *drag_origin.borrow();
            let (next_left, next_bottom, next_placement) = if mode.is_internal() {
                dragged_free_placement(origin, offset_x, offset_y)
            } else {
                dragged_placement(origin, offset_x, offset_y)
            };
            let next_top = y_from_bottom_margin(next_bottom, origin.geometry);

            if placement.position() == (next_left, next_top) {
                return;
            }
            placement.set(next_placement, next_left, next_top);
            if mode.is_internal() {
                stage.move_(&content, next_left as f64, next_top as f64);
            } else {
                window.set_margin(Edge::Left, next_left);
                window.set_margin(Edge::Bottom, next_bottom);
            }
            apply_snap_css_classes(&content, &next_placement);
        });
    }

    {
        let placement = placement.clone();
        let drag_handle = drag_handle.downgrade();
        gesture.connect_drag_end(move |_, _, _| {
            if let Some(drag_handle) = drag_handle.upgrade() {
                drag_handle.set_cursor_from_name(Some("grab"));
            }
            on_drag_end(placement.current().position());
        });
    }

    {
        let drag_handle = drag_handle.downgrade();
        gesture.connect_cancel(move |_, _| {
            if let Some(drag_handle) = drag_handle.upgrade() {
                drag_handle.set_cursor_from_name(Some("grab"));
            }
        });
    }

    if mode.is_internal() {
        // GestureDrag reports offsets in the controller widget's coordinate
        // space. Keep that widget stationary so moving the panel cannot feed
        // back into the next pointer offset during fast KDE drags.
        gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
        stage.add_controller(gesture);
    } else {
        content.add_controller(gesture);
    }
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
    let geometry = layer_surface_geometry(window, fallback_width, fallback_height)?;
    let y = vertical_position(placement, geometry.viewport_height, geometry.surface_height);
    Some(bottom_margin_from_y(y, geometry))
}

pub(super) fn left_position_for_width(
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

pub(super) fn reposition_for_width(
    window: &gtk::ApplicationWindow,
    stage: &gtk::Fixed,
    content: &gtk::Box,
    placement: &PlacementState,
    window_width: i32,
) {
    let Some(left) = left_position_for_width(window, &placement.current(), window_width) else {
        return;
    };
    let current = placement.current();
    if placement.uses_internal_drag() {
        let (current_left, top) = placement.position();
        if left == current_left {
            return;
        }
        stage.move_(content, left as f64, top as f64);
        placement.set(current, left, top);
    } else {
        window.set_margin(Edge::Left, left);
    }
}

pub(super) fn reposition_internal_content(
    window: &gtk::ApplicationWindow,
    stage: &gtk::Fixed,
    content: &gtk::Box,
    placement: &PlacementState,
    fallback_width: i32,
    fallback_height: i32,
) {
    if !placement.uses_internal_drag() {
        return;
    }
    let Some(geometry) = floating_geometry(
        window,
        content,
        DragMode::InternalPanel,
        fallback_width,
        fallback_height,
    ) else {
        return;
    };
    let current = placement.current();
    let left = horizontal_position(&current, geometry.viewport_width, geometry.surface_width);
    let top = vertical_position(&current, geometry.viewport_height, geometry.surface_height);
    stage.move_(content, left as f64, top as f64);
    placement.set(current, left, top);
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
    geometry: FloatingGeometry,
) -> WindowPlacement {
    placement_at(
        window.margin(Edge::Left),
        y_from_bottom_margin(window.margin(Edge::Bottom), geometry),
        geometry,
    )
}

fn floating_geometry(
    window: &gtk::ApplicationWindow,
    content: &gtk::Box,
    mode: DragMode,
    fallback_width: i32,
    fallback_height: i32,
) -> Option<FloatingGeometry> {
    if !mode.is_internal() {
        return layer_surface_geometry(window, fallback_width, fallback_height);
    }
    let monitor = window_monitor(window).or_else(first_monitor)?;
    let geometry = monitor.geometry();
    let surface_width = effective_surface_size(content.width(), fallback_width);
    let surface_height = effective_surface_size(content.height(), fallback_height);

    Some(FloatingGeometry {
        viewport_width: geometry.width().max(0),
        viewport_height: geometry.height().max(0),
        surface_width,
        surface_height,
    })
}

fn layer_surface_geometry(
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
