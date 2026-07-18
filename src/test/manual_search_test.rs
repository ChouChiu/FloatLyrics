use super::*;

#[test]
fn manual_search_fields_convert_spotify_metadata_to_simplified_chinese() {
    let track = TrackMetadata {
        title: "喜歡你".to_string(),
        artists: vec!["G.E.M.鄧紫棋".to_string()],
        album: None,
        duration_ms: None,
        mpris_track_id: None,
    };

    assert_eq!(
        search_field_values(&track),
        ("喜欢你".to_string(), "G.E.M.邓紫棋".to_string())
    );
}
