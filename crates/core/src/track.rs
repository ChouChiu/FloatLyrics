use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackMetadata {
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub mpris_track_id: Option<String>,
}

impl TrackMetadata {
    pub fn fingerprint(&self) -> String {
        track_fingerprint(
            &self.title,
            &self.artists,
            self.album.as_deref(),
            self.duration_ms,
        )
    }

    pub fn display_artist(&self) -> String {
        self.artists.join(", ")
    }

    pub fn playback_identity(&self) -> String {
        self.mpris_track_id
            .clone()
            .unwrap_or_else(|| self.fingerprint())
    }
}

pub fn track_fingerprint(
    title: &str,
    artists: &[String],
    album: Option<&str>,
    duration_ms: Option<u64>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonicalize(title).as_bytes());
    hasher.update(b"\0");

    let mut canonical_artists = artists
        .iter()
        .map(|artist| canonicalize(artist))
        .collect::<Vec<_>>();
    canonical_artists.sort();
    hasher.update(canonical_artists.join(";").as_bytes());
    hasher.update(b"\0");

    if let Some(album) = album {
        hasher.update(canonicalize(album).as_bytes());
    }
    hasher.update(b"\0");

    if let Some(duration_ms) = duration_ms {
        let rounded_seconds = (duration_ms + 500) / 1000;
        hasher.update(rounded_seconds.to_string().as_bytes());
    }

    format!("{:x}", hasher.finalize())
}

fn canonicalize(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_ignores_case_whitespace_and_artist_order() {
        let artists_a = vec!["Alice".to_string(), "Bob".to_string()];
        let artists_b = vec![" bob ".to_string(), "ALICE".to_string()];
        let a = track_fingerprint("  Song  Name ", &artists_a, Some(" Album "), Some(180_100));
        let b = track_fingerprint("song name", &artists_b, Some("album"), Some(180_400));

        assert_eq!(a, b);
    }

    #[test]
    fn playback_identity_prefers_the_stable_mpris_track_id() {
        let mut track = TrackMetadata {
            title: "Song".to_string(),
            artists: vec!["Artist".to_string()],
            album: None,
            duration_ms: None,
            mpris_track_id: Some("/org/mpris/MediaPlayer2/track/42".to_string()),
        };
        let identity = track.playback_identity();

        track.album = Some("Album loaded later".to_string());
        track.duration_ms = Some(180_000);

        assert_eq!(track.playback_identity(), identity);
    }
}
