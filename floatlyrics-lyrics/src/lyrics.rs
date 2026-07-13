// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! Lyrics domain facade.
//!
//! Public exports remain stable while models, parsing, provider search, and
//! playback timeline calculations are implemented independently.

mod model;
mod parsing;
mod romanization;
mod search;
mod timeline;

pub use lyrics_helper::{LineInfo, LyricsData, LyricsTypes, generate_string};
pub use model::{
    FetchedLyrics, LyricsCandidate, LyricsProvider, LyricsProviderParseError, RomanizationSegment,
    TimedLine, TimedSyllable,
};
pub use parsing::{
    combine_lyrics_with_translation, export_lyrics, parse_local_lyrics, timed_lines_from_data,
    timed_lines_from_raw,
};
pub use romanization::{
    ChineseRomanizationMode, generate_local_romanization, generate_local_romanization_with_mode,
};
pub use search::{
    SearchPlan, fetch_candidate_lyrics, search_best_lyrics, search_lyrics_candidates,
    simplify_search_text,
};
pub use timeline::{active_line_index, line_index_at_or_before};

/// Detects and parses a non-XML lyrics document.
///
/// Returns `None` for XML-based formats and unrecognized input.
#[must_use]
pub fn parse_auto(content: &str) -> Option<LyricsData> {
    parse_local_lyrics(content).ok()
}

#[cfg(test)]
#[path = "test/lyrics_test.rs"]
mod tests;
