// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! React manual-search coordinator and pure session state.

mod state;
pub(super) mod window;

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
        auto_preview: bool,
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
    pending_request: Option<tokio::task::JoinHandle<()>>,
    current_fingerprint: Option<String>,
    search_when_ready: bool,
    query_revision: u32,
}

impl ManualSearchCoordinator {
    pub(super) fn new(service: ManualSearchService, controller: ControllerHandle) -> Self {
        Self {
            service,
            controller,
            state: ManualSearchState::default(),
            title: String::new(),
            artist: String::new(),
            pending_request: None,
            current_fingerprint: None,
            search_when_ready: false,
            query_revision: 0,
        }
    }

    pub(super) fn observe_track(
        &mut self,
        fingerprint: Option<&str>,
        sender: &relm4::Sender<AppMsg>,
    ) -> bool {
        if self.current_fingerprint.as_deref() == fingerprint {
            return false;
        }
        self.current_fingerprint = fingerprint.map(str::to_owned);
        if self.search_when_ready && fingerprint.is_some() {
            self.prepare_and_search(sender.clone());
        } else {
            self.cancel_request();
            if fingerprint.is_some() {
                self.state.mark_track_changed();
            } else {
                self.state.reject_no_track();
            }
        }
        true
    }

    pub(super) fn prepare_and_search(&mut self, sender: relm4::Sender<AppMsg>) {
        if self.state.is_applying() {
            return;
        }
        if let Some(track) = self.controller.current_track() {
            self.current_fingerprint = Some(track.fingerprint());
            self.search_when_ready = false;
            self.query_revision = self.query_revision.wrapping_add(1);
            (self.title, self.artist) = ManualSearchService::search_field_values(&track);
            self.search(self.title.clone(), self.artist.clone(), sender);
        } else {
            self.search_when_ready = true;
            self.state.reject_no_track();
        }
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
        self.cancel_request();
        self.pending_request = Some(self.service.search(search_track, move |result| {
            let _ = sender.send(AppMsg::SearchEvent(SearchEvent::Candidates {
                generation,
                result,
                auto_preview: false,
            }));
        }));
    }

    pub(super) fn lookup_qq(&mut self, reference: String, sender: relm4::Sender<AppMsg>) {
        if self.state.is_applying() {
            return;
        }
        let Some(target_track) = self.controller.current_track() else {
            self.state.reject_no_track();
            return;
        };
        if reference.trim().is_empty() {
            self.state.reject_empty_qq_reference();
            return;
        }
        let Some(generation) = self.state.begin_search(target_track) else {
            return;
        };
        self.cancel_request();
        self.pending_request = Some(self.service.lookup_qq(reference, move |result| {
            let _ = sender.send(AppMsg::SearchEvent(SearchEvent::Candidates {
                generation,
                result,
                auto_preview: true,
            }));
        }));
    }

    pub(super) fn select(&mut self, index: usize, sender: relm4::Sender<AppMsg>) {
        let Some((generation, candidate)) = self.state.begin_preview(index) else {
            return;
        };
        self.cancel_request();
        self.pending_request = Some(self.service.preview(candidate, move |result| {
            let _ = sender.send(AppMsg::SearchEvent(SearchEvent::Preview {
                generation,
                index,
                result,
            }));
        }));
    }

    fn cancel_request(&mut self) {
        if let Some(request) = self.pending_request.take() {
            request.abort();
        }
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

    pub(super) fn handle_event(&mut self, event: SearchEvent, sender: relm4::Sender<AppMsg>) {
        match event {
            SearchEvent::Candidates {
                generation,
                result,
                auto_preview,
            } => {
                if self.state.accept_candidates(generation, result) && auto_preview {
                    self.select(0, sender);
                }
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
        let track = self.controller.current_track().map(|track| {
            if track.artists.is_empty() {
                return track.title;
            }
            format!("{} · {}", track.title, track.display_artist())
        });
        json!({
            "track": track,
            "query_revision": self.query_revision,
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
                "provider_track_id": candidate.provider_track_id,
                "title": candidate.title,
                "artists": candidate.artists,
                "album": candidate.album,
                "duration_ms": candidate.duration_ms,
                "match_score": candidate.match_score,
            })).collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
#[path = "manual_search/tests.rs"]
mod tests;
