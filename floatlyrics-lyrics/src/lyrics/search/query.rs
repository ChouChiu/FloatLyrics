// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Provider query normalization.

use floatlyrics_core::track::TrackMetadata;

pub(in crate::lyrics) fn lyrics_helper_metadata(
    track: &TrackMetadata,
) -> lyrics_helper::models::TrackMetadata {
    let mut metadata = lyrics_helper::models::TrackMetadata::new();
    metadata.title = Some(simplify_search_text(&track.title));
    metadata.artist = Some(simplify_search_text(&track.display_artist()));
    metadata.artists = Some(
        track
            .artists
            .iter()
            .map(|artist| simplify_search_text(artist))
            .collect(),
    );
    metadata.album = track.album.as_deref().map(simplify_search_text);
    metadata.duration_ms = track.duration_ms.and_then(|value| value.try_into().ok());
    metadata
}

/// Converts Traditional Chinese characters to Simplified Chinese for provider queries.
#[must_use]
pub fn simplify_search_text(text: &str) -> String {
    lyrics_helper::helpers::chinese_helper::to_simplified(text)
}
