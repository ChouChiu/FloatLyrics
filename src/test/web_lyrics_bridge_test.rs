use super::*;

#[test]
fn bridge_waits_for_readiness_and_coalesces_each_command_slot() {
    let mut bridge = BridgeState::default();
    bridge.enqueue(CommandSlot::Frame, "old-frame".to_string());
    bridge.enqueue(CommandSlot::Config, "config".to_string());
    bridge.enqueue(CommandSlot::Frame, "new-frame".to_string());

    assert_eq!(bridge.take_pending(), None);

    bridge.set_ready(true);
    assert_eq!(bridge.take_pending().as_deref(), Some("config\nnew-frame"));
    assert_eq!(bridge.take_pending(), None);

    bridge.enqueue(CommandSlot::Document, "document".to_string());
    bridge.complete_dispatch(true);
    assert_eq!(bridge.take_pending().as_deref(), Some("document"));
}

#[test]
fn bridge_restores_failed_or_interrupted_batches_without_overwriting_newer_slots() {
    let mut bridge = BridgeState::default();
    bridge.set_ready(true);
    bridge.enqueue(CommandSlot::Config, "old-config".to_string());
    bridge.enqueue(CommandSlot::Frame, "old-frame".to_string());
    assert_eq!(
        bridge.take_pending().as_deref(),
        Some("old-config\nold-frame")
    );

    bridge.enqueue(CommandSlot::Frame, "new-frame".to_string());
    bridge.complete_dispatch(false);
    assert_eq!(
        bridge.take_pending().as_deref(),
        Some("old-config\nnew-frame")
    );

    bridge.set_ready(false);
    bridge.set_ready(true);
    assert_eq!(
        bridge.take_pending().as_deref(),
        Some("old-config\nnew-frame")
    );
}

#[test]
fn bridge_dispatches_config_document_and_frame_in_dependency_order() {
    let mut bridge = BridgeState::default();
    bridge.set_ready(true);
    bridge.enqueue(CommandSlot::Frame, "frame".to_string());
    bridge.enqueue(CommandSlot::Document, "document".to_string());
    bridge.enqueue(CommandSlot::Config, "config".to_string());

    assert_eq!(
        bridge.take_pending().as_deref(),
        Some("config\ndocument\nframe")
    );
}
