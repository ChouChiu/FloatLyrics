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
