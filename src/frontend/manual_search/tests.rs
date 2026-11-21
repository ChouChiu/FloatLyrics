use super::*;

#[test]
fn manual_search_bridge_preserves_qq_song_reference() {
    use crate::frontend::messages::UiAction;

    let reference = "https://y.qq.com/n/ryqq/songDetail/733159779?songtype=0";
    let action = serde_json::from_value::<UiAction>(serde_json::json!({
        "type": "lookup-qq-lyrics",
        "reference": reference,
    }))
    .unwrap();

    assert!(matches!(action, UiAction::LookupQqLyrics { reference: value } if value == reference));
}

#[test]
fn manual_search_fields_convert_spotify_metadata_to_simplified_chinese() {
    let track = TrackMetadata {
        title: "喜歡你".to_string(),
        artists: vec!["G.E.M.鄧紫棋".to_string()],
        album: None,
        duration_ms: None,
        mpris_track_id: None,
        art_url: None,
    };

    assert_eq!(
        ManualSearchService::search_field_values(&track),
        ("喜欢你".to_string(), "G.E.M.邓紫棋".to_string())
    );
}
