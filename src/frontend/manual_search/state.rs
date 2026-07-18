// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Pure state and input normalization for one manual-search session.

use floatlyrics_core::{
    i18n::{Language, Text},
    track::TrackMetadata,
};

use crate::shared::manual_search::{FetchedLyrics, LyricsCandidate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SearchInputError {
    EmptyTitle,
}

pub(super) fn build_search_track(
    target: &TrackMetadata,
    title: &str,
    artists: &str,
) -> Result<TrackMetadata, SearchInputError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(SearchInputError::EmptyTitle);
    }

    Ok(TrackMetadata {
        title: title.to_string(),
        artists: artists
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect(),
        album: None,
        duration_ms: target.duration_ms,
        mpris_track_id: None,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ManualStatus {
    Text(Text),
    Detail(Text, String),
    CandidatesFound(usize),
}

impl ManualStatus {
    fn render(&self, language: Language) -> String {
        match self {
            Self::Text(key) => language.text(*key).to_string(),
            Self::Detail(key, detail) => language.detail(*key, detail),
            Self::CandidatesFound(count) => language.candidates_found(*count),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PreviewState {
    Text(Text),
    Lyrics(String),
}

impl PreviewState {
    fn render(&self, language: Language) -> String {
        match self {
            Self::Text(key) => language.text(*key).to_string(),
            Self::Lyrics(lyrics) => lyrics.clone(),
        }
    }
}

pub(super) struct ManualSearchState {
    generation: u64,
    target_track: Option<TrackMetadata>,
    candidates: Vec<LyricsCandidate>,
    preview_index: Option<usize>,
    selected: Option<(usize, FetchedLyrics)>,
    applying_generation: Option<u64>,
    status: ManualStatus,
    preview: PreviewState,
    searching: bool,
}

impl Default for ManualSearchState {
    fn default() -> Self {
        Self {
            generation: 0,
            target_track: None,
            candidates: Vec::new(),
            preview_index: None,
            selected: None,
            applying_generation: None,
            status: ManualStatus::Text(Text::SearchAfterPlayback),
            preview: PreviewState::Text(Text::SelectCandidatePreview),
            searching: false,
        }
    }
}

impl ManualSearchState {
    pub(super) fn begin_search(&mut self, target_track: TrackMetadata) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.target_track = Some(target_track);
        self.candidates.clear();
        self.preview_index = None;
        self.selected = None;
        self.applying_generation = None;
        self.status = ManualStatus::Text(Text::SearchingProviders);
        self.preview = PreviewState::Text(Text::SearchingCandidates);
        self.searching = true;
        self.generation
    }

    pub(super) fn reject_no_track(&mut self) {
        self.status = ManualStatus::Text(Text::NoTrackPlaying);
    }

    pub(super) fn reject_empty_title(&mut self) {
        self.status = ManualStatus::Text(Text::EnterSongTitle);
    }

    pub(super) fn accept_candidates(
        &mut self,
        generation: u64,
        result: Result<Vec<LyricsCandidate>, String>,
    ) -> bool {
        if self.generation != generation {
            return false;
        }

        self.searching = false;
        self.preview_index = None;
        self.selected = None;
        self.applying_generation = None;
        match result {
            Ok(candidates) => {
                self.candidates = candidates;
                if self.candidates.is_empty() {
                    self.status = ManualStatus::Text(Text::NoCandidates);
                    self.preview = PreviewState::Text(Text::NoCandidates);
                } else {
                    self.status = ManualStatus::CandidatesFound(self.candidates.len());
                }
            }
            Err(error) => {
                self.candidates.clear();
                self.status = ManualStatus::Detail(Text::SearchFailed, error);
                self.preview = PreviewState::Text(Text::LyricsSearchPreviewFailed);
            }
        }
        true
    }

    pub(super) fn begin_preview(&mut self, index: usize) -> Option<(u64, LyricsCandidate)> {
        let candidate = self.candidates.get(index)?.clone();
        self.selected = None;
        self.applying_generation = None;
        self.preview_index = Some(index);
        self.status = ManualStatus::Text(Text::LoadingPreview);
        self.preview = PreviewState::Text(Text::LoadingPreview);
        Some((self.generation, candidate))
    }

    pub(super) fn accept_preview(
        &mut self,
        generation: u64,
        index: usize,
        result: Result<Option<FetchedLyrics>, String>,
    ) -> bool {
        if self.generation != generation || self.preview_index != Some(index) {
            return false;
        }

        self.selected = None;
        match result {
            Ok(Some(fetched)) => {
                self.preview = PreviewState::Lyrics(fetched.raw_lyrics.clone());
                self.selected = Some((index, fetched));
                self.status = ManualStatus::Text(Text::PreviewReady);
            }
            Ok(None) => {
                self.preview = PreviewState::Text(Text::CandidateUnavailable);
                self.status = ManualStatus::Text(Text::CandidateUnavailable);
            }
            Err(error) => {
                self.preview = PreviewState::Text(Text::PreviewLoadFailed);
                self.status = ManualStatus::Detail(Text::LoadingFailed, error);
            }
        }
        true
    }

    pub(super) fn selection(&self) -> Option<(&TrackMetadata, &FetchedLyrics)> {
        let target = self.target_track.as_ref()?;
        let (_, fetched) = self.selected.as_ref()?;
        Some((target, fetched))
    }

    pub(super) fn begin_apply(&mut self) -> Option<(u64, TrackMetadata, FetchedLyrics)> {
        if self.applying_generation.is_some() {
            return None;
        }
        let target = self.target_track.clone()?;
        let (_, fetched) = self.selected.as_ref()?;
        self.applying_generation = Some(self.generation);
        Some((self.generation, target, fetched.clone()))
    }

    pub(super) fn is_current_apply(&self, generation: u64) -> bool {
        self.generation == generation && self.applying_generation == Some(generation)
    }

    pub(super) fn mark_track_changed(&mut self) {
        self.applying_generation = None;
        self.selected = None;
        self.status = ManualStatus::Text(Text::TrackChanged);
    }

    pub(super) fn finish_apply(&mut self, generation: u64, result: Result<(), String>) -> bool {
        if self.applying_generation != Some(generation) || self.generation != generation {
            return false;
        }
        self.applying_generation = None;
        self.status = match result {
            Ok(()) => {
                self.selected = None;
                ManualStatus::Text(Text::LyricsApplied)
            }
            Err(error) => ManualStatus::Detail(Text::ApplyFailed, error),
        };
        true
    }

    pub(super) fn presentation_snapshot(&self) -> (Vec<LyricsCandidate>, Option<usize>) {
        (self.candidates.clone(), self.preview_index)
    }

    pub(super) fn status_text(&self, language: Language) -> String {
        self.status.render(language)
    }

    pub(super) fn preview_text(&self, language: Language) -> String {
        self.preview.render(language)
    }

    pub(super) fn is_searching(&self) -> bool {
        self.searching
    }

    pub(super) fn can_apply(&self) -> bool {
        self.selected.is_some() && self.applying_generation.is_none()
    }
}

#[cfg(test)]
#[path = "../../test/manual_search_state_test.rs"]
mod tests;
