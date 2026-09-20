// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! GTK-independent lyrics presentation contracts.

use serde::{Deserialize, Serialize};

use floatlyrics_lyrics::lyrics::{TimedSyllable, Voice};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct KaraokeRenderState {
    pub(crate) text: String,
    pub(crate) syllables: Vec<TimedSyllable>,
    pub(crate) position_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct LyricSlotText {
    pub(crate) text: String,
    pub(crate) karaoke: Option<KaraokeRenderState>,
    pub(crate) romanization: String,
    pub(crate) translation: String,
}

impl LyricSlotText {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn message(message: &str) -> Self {
        Self {
            text: message.to_string(),
            karaoke: None,
            romanization: String::new(),
            translation: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct PresentedLyricLine {
    pub(crate) start_ms: u64,
    pub(crate) end_ms: Option<u64>,
    pub(crate) text: String,
    pub(crate) syllables: Vec<TimedSyllable>,
    pub(crate) romanization: String,
    pub(crate) translation: String,
    pub(crate) background: String,
    /// Translation of the background vocal, empty when it has none.
    ///
    /// Only the AMLL sender reads it, writing it inside the span of the
    /// background vocal it belongs to.
    pub(crate) background_translation: String,
    /// Where the background vocal is sung within the track.
    pub(crate) background_start_ms: u64,
    /// Exclusive end of the background vocal, when the provider timed one.
    pub(crate) background_end_ms: Option<u64>,
    /// Words of the background vocal, when the provider timed them.
    ///
    /// The AMLL sender writes them as the spans inside the background vocal, so
    /// a listener fills the words it hears as it hears them rather than one long
    /// span.
    pub(crate) background_syllables: Vec<TimedSyllable>,
    /// Which vocal part sings this line.
    ///
    /// Only the AMLL sender reads it, to write the performer each line belongs to
    /// into the TTML document it streams.
    pub(crate) voice: Voice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct LyricsDocument {
    pub(crate) revision: u64,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) lines: Vec<PresentedLyricLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct LyricsFrame {
    pub(crate) key: String,
    pub(crate) content: LyricSlotText,
    pub(crate) position_ms: Option<u64>,
    pub(crate) playing: bool,
    pub(crate) seeking: bool,
}

/// Loop mode of the active player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum LoopStatus {
    /// Playback stops after the current track.
    Off,
    /// The current track repeats.
    Track,
    /// The whole playlist repeats.
    Playlist,
}

/// Playback actions the active player reports as available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PlayerCapabilities {
    pub(crate) can_control: bool,
    pub(crate) can_play: bool,
    pub(crate) can_pause: bool,
    pub(crate) can_seek: bool,
    pub(crate) can_next: bool,
    pub(crate) can_previous: bool,
}

impl Default for PlayerCapabilities {
    /// Everything is assumed available, which is how a player that does not
    /// report its capabilities is treated.
    fn default() -> Self {
        Self {
            can_control: true,
            can_play: true,
            can_pause: true,
            can_seek: true,
            can_next: true,
            can_previous: true,
        }
    }
}

/// Volume, playback modes, and capabilities of the active player.
///
/// A missing value means the player did not report that property, so the
/// corresponding control is hidden rather than shown as an unknown value.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub(crate) struct PlayerControl {
    /// Player volume in `0.0..=1.0`, when the player reports one.
    pub(crate) volume: Option<f64>,
    pub(crate) loop_status: Option<LoopStatus>,
    pub(crate) shuffle: Option<bool>,
    pub(crate) capabilities: PlayerCapabilities,
}
