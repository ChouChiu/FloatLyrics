// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! React manual-search coordinator and pure session state.

mod state;

use floatlyrics_core::{i18n::Language, track::TrackMetadata};
use serde_json::{Value, json};

use crate::{
    backend::{ControllerHandle, ManualSearchService},
    shared::manual_search::{FetchedLyrics, LyricsCandidate},
};

use super::AppMsg;
use state::{ManualSearchState, SearchInputError, build_search_track};

#[derive(Debug)]
pub(super) enum SearchEvent {
    Candidates {
        generation: u64,
        result: Result<Vec<LyricsCandidate>, String>,
    },
    Preview {
        generation: u64,
        index: usize,
        result: Result<Option<FetchedLyrics>, String>,
    },
    Applied {
        generation: u64,
        target_fingerprint: String,
        result: Result<(), String>,
    },
}

pub(super) struct ManualSearchCoordinator {
    service: ManualSearchService,
    controller: ControllerHandle,
    state: ManualSearchState,
    title: String,
    artist: String,
}

impl ManualSearchCoordinator {
    pub(super) fn new(service: ManualSearchService, controller: ControllerHandle) -> Self {
        Self {
            service,
            controller,
            state: ManualSearchState::default(),
            title: String::new(),
            artist: String::new(),
        }
    }

    pub(super) fn prepare_and_search(&mut self, sender: relm4::Sender<AppMsg>) {
        if let Some(track) = self.controller.current_track() {
            (self.title, self.artist) = search_field_values(&track);
        }
        self.search(self.title.clone(), self.artist.clone(), sender);
    }

    pub(super) fn search(&mut self, title: String, artist: String, sender: relm4::Sender<AppMsg>) {
        if self.state.is_applying() {
            return;
        }
        self.title = title;
        self.artist = artist;
        let Some(target_track) = self.controller.current_track() else {
            self.state.reject_no_track();
            return;
        };
        let search_track = match build_search_track(&target_track, &self.title, &self.artist) {
            Ok(track) => track,
            Err(SearchInputError::EmptyTitle) => {
                self.state.reject_empty_title();
                return;
            }
        };
        let Some(generation) = self.state.begin_search(target_track) else {
            return;
        };
        self.service.search(search_track, move |result| {
            let _ = sender.send(AppMsg::SearchEvent(SearchEvent::Candidates {
                generation,
                result,
            }));
        });
    }

    pub(super) fn select(&mut self, index: usize, sender: relm4::Sender<AppMsg>) {
        let Some((generation, candidate)) = self.state.begin_preview(index) else {
            return;
        };
        self.service.preview(candidate, move |result| {
            let _ = sender.send(AppMsg::SearchEvent(SearchEvent::Preview {
                generation,
                index,
                result,
            }));
        });
    }

    pub(super) fn apply(&mut self, sender: relm4::Sender<AppMsg>) {
        let target_fingerprint = self
            .state
            .selection()
            .map(|(target, _)| target.fingerprint());
        let Some(target_fingerprint) = target_fingerprint else {
            return;
        };
        if self
            .controller
            .current_track()
            .as_ref()
            .map(TrackMetadata::fingerprint)
            != Some(target_fingerprint.clone())
        {
            self.state.mark_track_changed();
            return;
        }
        let Some((generation, target, fetched)) = self.state.begin_apply() else {
            return;
        };
        self.service.apply(target, fetched, move |result| {
            let _ = sender.send(AppMsg::SearchEvent(SearchEvent::Applied {
                generation,
                target_fingerprint,
                result,
            }));
        });
    }

    pub(super) fn handle_event(&mut self, event: SearchEvent) {
        match event {
            SearchEvent::Candidates { generation, result } => {
                self.state.accept_candidates(generation, result);
            }
            SearchEvent::Preview {
                generation,
                index,
                result,
            } => {
                self.state.accept_preview(generation, index, result);
            }
            SearchEvent::Applied {
                generation,
                target_fingerprint,
                result,
            } => {
                if !self.state.is_current_apply(generation) {
                    return;
                }
                let current_matches = self
                    .controller
                    .current_track()
                    .is_some_and(|track| track.fingerprint() == target_fingerprint);
                if !current_matches {
                    self.state.mark_track_changed();
                    return;
                }
                let succeeded = result.is_ok();
                if self.state.finish_apply(generation, result) && succeeded {
                    self.controller.reload_lyrics();
                }
            }
        }
    }

    pub(super) fn snapshot(&self, language: Language) -> Value {
        let (candidates, selected_index) = self.state.presentation_snapshot();
        json!({
            "title": self.title,
            "artist": self.artist,
            "status": self.state.status_text(language),
            "preview": self.state.preview_text(language),
            "searching": self.state.is_searching(),
            "applying": self.state.is_applying(),
            "can_apply": self.state.can_apply(),
            "selected_index": selected_index,
            "candidates": candidates.iter().map(|candidate| json!({
                "provider": candidate.provider.as_str(),
                "title": candidate.title,
                "artists": candidate.artists,
                "album": candidate.album,
                "duration_ms": candidate.duration_ms,
                "match_score": candidate.match_score,
            })).collect::<Vec<_>>(),
        })
    }
}

fn search_field_values(track: &TrackMetadata) -> (String, String) {
    ManualSearchService::search_field_values(track)
}

#[cfg(test)]
#[path = "../test/manual_search_test.rs"]
mod tests;
