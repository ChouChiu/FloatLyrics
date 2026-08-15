use super::*;

#[test]
fn bridge_waits_for_readiness_and_coalesces_each_command_slot() {
    let mut bridge = BridgeState::default();
    bridge.enqueue(
        CommandSlot::Frame {
            seeking: false,
            document_revision: None,
        },
        "old-frame".to_string(),
    );
    bridge.enqueue(CommandSlot::Config, "config".to_string());
    bridge.enqueue(
        CommandSlot::Frame {
            seeking: false,
            document_revision: None,
        },
        "new-frame".to_string(),
    );

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
    bridge.enqueue(
        CommandSlot::Frame {
            seeking: false,
            document_revision: None,
        },
        "old-frame".to_string(),
    );
    assert_eq!(
        bridge.take_pending().as_deref(),
        Some("old-config\nold-frame")
    );

    bridge.enqueue(
        CommandSlot::Frame {
            seeking: false,
            document_revision: None,
        },
        "new-frame".to_string(),
    );
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
    bridge.enqueue(
        CommandSlot::Frame {
            seeking: false,
            document_revision: None,
        },
        "frame".to_string(),
    );
    bridge.enqueue(CommandSlot::Document, "document".to_string());
    bridge.enqueue(CommandSlot::Config, "config".to_string());

    assert_eq!(
        bridge.take_pending().as_deref(),
        Some("config\ndocument\nframe")
    );
}

#[test]
fn bridge_keeps_independent_react_shell_state_commands() {
    let mut bridge = BridgeState::default();
    bridge.enqueue(CommandSlot::Navigation, "navigate".to_string());
    bridge.enqueue(CommandSlot::SearchState, "search".to_string());
    bridge.enqueue(CommandSlot::ConfigState, "config-state".to_string());
    bridge.enqueue(CommandSlot::OverlayPlacement, "old-placement".to_string());
    bridge.enqueue(CommandSlot::OverlayPlacement, "placement".to_string());
    bridge.enqueue(CommandSlot::OverlayAppearance, "old-appearance".to_string());
    bridge.enqueue(CommandSlot::OverlayAppearance, "appearance".to_string());
    bridge.set_ready(true);

    assert_eq!(
        bridge.take_pending().as_deref(),
        Some("config-state\nsearch\nnavigate\nplacement\nappearance")
    );
}

#[test]
fn bridge_does_not_coalesce_away_a_pending_seek_frame() {
    let mut bridge = BridgeState::default();
    bridge.set_ready(true);
    bridge.enqueue(
        CommandSlot::Frame {
            seeking: false,
            document_revision: Some(1),
        },
        "in-flight".to_string(),
    );
    assert_eq!(bridge.take_pending().as_deref(), Some("in-flight"));

    bridge.enqueue(
        CommandSlot::Frame {
            seeking: true,
            document_revision: Some(1),
        },
        "seek".to_string(),
    );
    bridge.enqueue(
        CommandSlot::Frame {
            seeking: false,
            document_revision: Some(1),
        },
        "newer-frame".to_string(),
    );
    bridge.complete_dispatch(true);

    assert_eq!(bridge.take_pending().as_deref(), Some("seek"));
}

#[test]
fn bridge_allows_a_new_document_frame_to_replace_an_old_seek() {
    let mut bridge = BridgeState::default();
    bridge.enqueue(
        CommandSlot::Frame {
            seeking: true,
            document_revision: Some(1),
        },
        "old-seek".to_string(),
    );
    bridge.enqueue(
        CommandSlot::Frame {
            seeking: false,
            document_revision: Some(2),
        },
        "new-document-frame".to_string(),
    );
    bridge.set_ready(true);

    assert_eq!(bridge.take_pending().as_deref(), Some("new-document-frame"));
}
