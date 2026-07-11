// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

use std::time::{Duration, Instant};

use crate::track::TrackMetadata;

use super::model::{PlaybackStatus, SpotifyPlayerState};

const NEW_TRACK_POSITION_TOLERANCE: Duration = Duration::from_millis(1_500);
const TRACK_CHANGE_GRACE_PERIOD: Duration = Duration::from_secs(3);

pub(super) fn position_us_to_ms(position_us: i64) -> Option<u64> {
    if position_us >= 0 {
        Some(position_us as u64 / 1_000)
    } else {
        None
    }
}

pub(super) fn player_track_identity(state: &SpotifyPlayerState) -> Option<String> {
    state.track.as_ref().map(TrackMetadata::playback_identity)
}

#[derive(Debug, Clone)]
pub(super) struct TrackPositionSync {
    track_identity: Option<String>,
    detected_at: Instant,
    pub(super) synchronized: bool,
}

impl TrackPositionSync {
    pub(super) fn new(state: &SpotifyPlayerState, now: Instant) -> Self {
        Self {
            track_identity: player_track_identity(state),
            detected_at: now,
            synchronized: true,
        }
    }

    pub(super) fn observe_track(&mut self, state: &SpotifyPlayerState, now: Instant) -> bool {
        let identity = player_track_identity(state);
        if self.track_identity == identity {
            return false;
        }

        self.track_identity = identity;
        self.detected_at = now;
        self.synchronized = false;
        true
    }

    pub(super) fn accepts(
        &mut self,
        position_ms: Option<u64>,
        playback_status: &PlaybackStatus,
        now: Instant,
    ) -> bool {
        let Some(position_ms) = position_ms else {
            return false;
        };
        if self.synchronized || !matches!(playback_status, PlaybackStatus::Playing) {
            self.synchronized = true;
            return true;
        }

        let elapsed_ms = now.duration_since(self.detected_at).as_millis() as u64;
        let tolerance_ms = NEW_TRACK_POSITION_TOLERANCE.as_millis() as u64;
        let grace_ms = TRACK_CHANGE_GRACE_PERIOD.as_millis() as u64;
        if position_ms <= elapsed_ms.saturating_add(tolerance_ms) || elapsed_ms > grace_ms {
            self.synchronized = true;
            return true;
        }

        false
    }

    pub(super) fn trust_position(&mut self) {
        self.synchronized = true;
    }

    pub(super) fn estimated_position(&self, now: Instant) -> Option<u64> {
        (!self.synchronized).then(|| now.duration_since(self.detected_at).as_millis() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::TrackMetadata;

    fn state_with_id(track_id: &str) -> SpotifyPlayerState {
        SpotifyPlayerState {
            bus_name: "test".into(),
            playback_status: PlaybackStatus::Playing,
            position_ms: Some(0),
            track: Some(TrackMetadata {
                title: "Test".into(),
                artists: vec![],
                album: None,
                duration_ms: None,
                mpris_track_id: Some(track_id.to_string()),
            }),
        }
    }

    fn make_sync() -> TrackPositionSync {
        TrackPositionSync::new(&state_with_id("track_a"), Instant::now())
    }

    #[test]
    fn accepts_position_within_tolerance() {
        let mut sync = make_sync();
        let now = Instant::now();
        assert!(sync.observe_track(&state_with_id("track_b"), now));
        assert!(sync.accepts(
            Some(500),
            &PlaybackStatus::Playing,
            now + Duration::from_millis(100),
        ));
        assert!(sync.synchronized);
    }

    #[test]
    fn accepts_position_after_grace_period() {
        let mut sync = make_sync();
        let now = Instant::now();
        assert!(sync.observe_track(&state_with_id("track_b"), now));

        // Position far ahead — simulates stale old-track D-Bus value.
        let stale = 180_000u64;
        assert!(!sync.accepts(
            Some(stale),
            &PlaybackStatus::Playing,
            now + Duration::from_millis(500),
        ));

        // Still rejected when position and elapsed advance together.
        assert!(!sync.accepts(
            Some(stale + 1000),
            &PlaybackStatus::Playing,
            now + Duration::from_millis(1500),
        ));

        // After grace period (3s), should accept unconditionally.
        assert!(sync.accepts(
            Some(stale + 5000),
            &PlaybackStatus::Playing,
            now + Duration::from_secs(4),
        ));
        assert!(sync.synchronized);
    }

    #[test]
    fn rejects_none_position() {
        let mut sync = make_sync();
        let now = Instant::now();
        assert!(sync.observe_track(&state_with_id("track_b"), now));
        assert!(!sync.accepts(None, &PlaybackStatus::Playing, now));
    }

    #[test]
    fn accepts_when_paused_regardless_of_position() {
        let mut sync = make_sync();
        let now = Instant::now();
        assert!(sync.observe_track(&state_with_id("track_b"), now));
        assert!(sync.accepts(Some(999_999), &PlaybackStatus::Paused, now));
        assert!(sync.synchronized);
    }
}
