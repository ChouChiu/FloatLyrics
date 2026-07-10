use std::time::{Duration, Instant};

use crate::track::TrackMetadata;

use super::model::{PlaybackStatus, SpotifyPlayerState};

const NEW_TRACK_POSITION_TOLERANCE: Duration = Duration::from_millis(1_500);

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
        if position_ms <= elapsed_ms.saturating_add(tolerance_ms) {
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
