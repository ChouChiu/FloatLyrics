use super::*;

#[test]
fn track_offset_label_uses_the_localized_unit_and_unicode_minus() {
    assert_eq!(track_offset_label(300, "milliseconds"), "+300 milliseconds");
    assert_eq!(track_offset_label(-300, "毫秒"), "−300 毫秒");
    assert_eq!(track_offset_label(0, "ms"), "0 ms");
}

#[test]
fn kde_uses_internal_drag_to_bypass_compositor_move_effects() {
    assert_eq!(desktop_drag_mode(Some("KDE")), DragMode::InternalPanel);
    assert_eq!(
        desktop_drag_mode(Some("ubuntu:plasma")),
        DragMode::InternalPanel
    );
    assert_eq!(desktop_drag_mode(Some("GNOME")), DragMode::LayerSurface);
    assert_eq!(desktop_drag_mode(None), DragMode::LayerSurface);
}

#[test]
fn compositor_without_layer_shell_falls_back_to_a_plain_window() {
    assert_eq!(
        overlay_drag_mode(false, Some("KDE")),
        DragMode::ToplevelWindow
    );
    assert_eq!(
        overlay_drag_mode(false, Some("GNOME")),
        DragMode::ToplevelWindow
    );
    assert_eq!(overlay_drag_mode(false, None), DragMode::ToplevelWindow);
    assert_eq!(
        overlay_drag_mode(true, Some("KDE")),
        DragMode::InternalPanel
    );
    assert_eq!(
        overlay_drag_mode(true, Some("GNOME")),
        DragMode::LayerSurface
    );
}
