// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Pure placement, snapping, and coordinate conversion geometry.

use crate::shared::config::WindowPosition;

const SNAP_THRESHOLD_PX: i32 = 12;

#[derive(Debug, Clone, Copy, PartialEq)]
enum AxisAnchor {
    Start,
    Center,
    End,
    Free(f64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::frontend::view) struct WindowPlacement {
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

impl WindowPlacement {
    pub(in crate::frontend::view) fn from_position(position: WindowPosition) -> Self {
        Self {
            horizontal: anchor_from_factor(position.horizontal),
            vertical: anchor_from_factor(position.vertical),
        }
    }

    pub(super) fn position(self) -> WindowPosition {
        WindowPosition {
            horizontal: anchor_factor(self.horizontal),
            vertical: anchor_factor(self.vertical),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct DragOrigin {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) geometry: FloatingGeometry,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct FloatingGeometry {
    pub(super) viewport_width: i32,
    pub(super) viewport_height: i32,
    pub(super) surface_width: i32,
    pub(super) surface_height: i32,
}

pub(super) fn dragged_placement(
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

pub(super) fn placement_at(x: i32, y: i32, geometry: FloatingGeometry) -> WindowPlacement {
    WindowPlacement {
        horizontal: snap_axis(x, geometry.surface_width, geometry.viewport_width).1,
        vertical: snap_axis(y, geometry.surface_height, geometry.viewport_height).1,
    }
}

fn anchor_from_factor(factor: f64) -> AxisAnchor {
    let factor = if factor.is_finite() {
        factor.clamp(0.0, 1.0)
    } else {
        0.5
    };
    if factor == 0.0 {
        AxisAnchor::Start
    } else if factor == 0.5 {
        AxisAnchor::Center
    } else if factor == 1.0 {
        AxisAnchor::End
    } else {
        AxisAnchor::Free(factor)
    }
}

fn anchor_factor(anchor: AxisAnchor) -> f64 {
    match anchor {
        AxisAnchor::Start => 0.0,
        AxisAnchor::Center => 0.5,
        AxisAnchor::End => 1.0,
        AxisAnchor::Free(factor) => factor.clamp(0.0, 1.0),
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

pub(super) fn horizontal_position(
    placement: &WindowPlacement,
    viewport_size: i32,
    surface_size: i32,
) -> i32 {
    position_for_anchor(placement.horizontal, viewport_size, surface_size)
}

pub(super) fn vertical_position(
    placement: &WindowPlacement,
    viewport_size: i32,
    surface_size: i32,
) -> i32 {
    position_for_anchor(placement.vertical, viewport_size, surface_size)
}

pub(super) fn centered_position(viewport_size: i32, surface_size: i32) -> i32 {
    position_for_anchor(AxisAnchor::Center, viewport_size, surface_size)
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

pub(super) fn fallback_geometry(fallback_width: i32, fallback_height: i32) -> FloatingGeometry {
    FloatingGeometry {
        viewport_width: fallback_width.max(0),
        viewport_height: fallback_height.max(0),
        surface_width: fallback_width.max(0),
        surface_height: fallback_height.max(0),
    }
}

pub(super) fn effective_surface_size(actual: i32, fallback: i32) -> i32 {
    if actual > 0 { actual } else { fallback.max(0) }
}

pub(super) fn y_from_bottom_margin(bottom_margin: i32, geometry: FloatingGeometry) -> i32 {
    (geometry.viewport_height - geometry.surface_height - bottom_margin).clamp(
        0,
        maximum_position(geometry.viewport_height, geometry.surface_height),
    )
}

pub(super) fn bottom_margin_from_y(y: i32, geometry: FloatingGeometry) -> i32 {
    (geometry.viewport_height - geometry.surface_height - y).clamp(
        0,
        maximum_position(geometry.viewport_height, geometry.surface_height),
    )
}

pub(super) fn snap_css_classes(placement: &WindowPlacement) -> Vec<&'static str> {
    let mut classes = Vec::new();
    match placement.horizontal {
        AxisAnchor::Start => classes.push("snapped-left"),
        AxisAnchor::End => classes.push("snapped-right"),
        _ => {}
    }
    match placement.vertical {
        AxisAnchor::Start => classes.push("snapped-top"),
        AxisAnchor::End => classes.push("snapped-bottom"),
        _ => {}
    }
    classes
}

#[cfg(test)]
#[path = "../../../test/positioning_test.rs"]
mod tests;
