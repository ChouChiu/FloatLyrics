// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Message-driven manual-search orchestration and GTK presentation state.

use std::{cell::Cell, rc::Rc};

use floatlyrics_core::i18n::{I18n, Language};
use gtk::prelude::*;

use crate::backend::{ControllerHandle, ManualSearchService};

use super::{
    ManualSearchMsg, SearchEvent, search_field_values,
    state::{ManualSearchState, SearchInputError, build_search_track},
    view::candidate_row,
};

pub(super) struct SearchWidgets {
    pub(super) title: gtk::Entry,
    pub(super) artist: gtk::Entry,
    pub(super) results: gtk::ListBox,
    pub(super) preview: gtk::TextView,
    pub(super) status: gtk::Label,
    pub(super) apply: gtk::Button,
    pub(super) spinner: gtk::Spinner,
    pub(super) search_button: gtk::Button,
}

pub(super) struct ManualSearchSession {
    service: ManualSearchService,
    controller: ControllerHandle,
    sender: relm4::Sender<ManualSearchMsg>,
    widgets: SearchWidgets,
    state: ManualSearchState,
    language: Language,
    rebuilding_results: Rc<Cell<bool>>,
}

impl ManualSearchSession {
    pub(super) fn new(
        service: ManualSearchService,
        controller: ControllerHandle,
        i18n: &I18n,
        sender: relm4::Sender<ManualSearchMsg>,
        widgets: SearchWidgets,
    ) -> Self {
        let rebuilding_results = Rc::new(Cell::new(false));
        {
            let sender = sender.clone();
            let rebuilding_results = Rc::clone(&rebuilding_results);
            widgets.results.connect_row_selected(move |_, row| {
                if rebuilding_results.get() {
                    return;
                }
                let index = row.and_then(|row| usize::try_from(row.index()).ok());
                let _ = sender.send(ManualSearchMsg::SelectRow(index));
            });
        }
        {
            let sender = sender.clone();
            i18n.subscribe(move |language| {
                let _ = sender.send(ManualSearchMsg::LanguageChanged(language));
            });
        }

        let session = Self {
            service,
            controller,
            sender,
            widgets,
            state: ManualSearchState::default(),
            language: i18n.language(),
            rebuilding_results,
        };
        session.render_state();
        session
    }

    pub(super) fn prepare_for_show(&self) {
        if let Some(track) = self.controller.current_track() {
            let (title, artist) = search_field_values(&track);
            self.widgets.title.set_text(&title);
            self.widgets.artist.set_text(&artist);
        }
    }

    pub(super) fn start_search(&mut self) {
        let Some(target_track) = self.controller.current_track() else {
            self.state.reject_no_track();
            self.render_state();
            return;
        };
        let title = self.widgets.title.text();
        let artists = self.widgets.artist.text();
        let search_track = match build_search_track(&target_track, &title, &artists) {
            Ok(track) => track,
            Err(SearchInputError::EmptyTitle) => {
                self.state.reject_empty_title();
                self.render_state();
                return;
            }
        };
        let generation = self.state.begin_search(target_track);
        self.clear_results();
        self.render_state();

        let sender = self.sender.clone();
        self.service.search(search_track, move |result| {
            let _ = sender.send(ManualSearchMsg::Event(SearchEvent::Candidates {
                generation,
                result,
            }));
        });
    }

    pub(super) fn apply_selected(&mut self) {
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
            .map(|track| track.fingerprint())
            != Some(target_fingerprint.clone())
        {
            self.state.mark_track_changed();
            self.render_state();
            return;
        }
        let Some((generation, target, fetched)) = self.state.begin_apply() else {
            return;
        };
        self.render_state();
        let sender = self.sender.clone();
        self.service.apply(target, fetched, move |result| {
            let _ = sender.send(ManualSearchMsg::Event(SearchEvent::Applied {
                generation,
                target_fingerprint,
                result,
            }));
        });
    }

    pub(super) fn handle_event(&mut self, event: SearchEvent) {
        match event {
            SearchEvent::Candidates { generation, result } => {
                self.handle_candidates(generation, result);
            }
            SearchEvent::Preview {
                generation,
                index,
                result,
            } => self.handle_preview(generation, index, result),
            SearchEvent::Applied {
                generation,
                target_fingerprint,
                result,
            } => self.handle_applied(generation, &target_fingerprint, result),
        }
    }

    pub(super) fn select_row(&mut self, index: Option<usize>) {
        let Some(index) = index else {
            return;
        };
        let Some((generation, candidate)) = self.state.begin_preview(index) else {
            return;
        };
        self.render_state();

        let sender = self.sender.clone();
        self.service.preview(candidate, move |result| {
            let _ = sender.send(ManualSearchMsg::Event(SearchEvent::Preview {
                generation,
                index,
                result,
            }));
        });
    }

    pub(super) fn relocalize(&mut self, language: Language) {
        self.language = language;
        self.render_state();
        let (candidates, selected) = self.state.presentation_snapshot();
        self.rebuilding_results.set(true);
        self.clear_results();
        for candidate in &candidates {
            self.widgets
                .results
                .append(&candidate_row(candidate, language));
        }
        if let Some(index) = selected.and_then(|index| i32::try_from(index).ok())
            && let Some(row) = self.widgets.results.row_at_index(index)
        {
            self.widgets.results.select_row(Some(&row));
        }
        self.rebuilding_results.set(false);
    }

    fn handle_candidates(
        &mut self,
        generation: u64,
        result: Result<Vec<crate::shared::manual_search::LyricsCandidate>, String>,
    ) {
        if !self.state.accept_candidates(generation, result) {
            return;
        }
        let (candidates, _) = self.state.presentation_snapshot();
        for candidate in &candidates {
            self.widgets
                .results
                .append(&candidate_row(candidate, self.language));
        }
        self.render_state();
        if let Some(row) = self.widgets.results.row_at_index(0) {
            self.widgets.results.select_row(Some(&row));
        }
    }

    fn handle_preview(
        &mut self,
        generation: u64,
        index: usize,
        result: Result<Option<crate::shared::manual_search::FetchedLyrics>, String>,
    ) {
        if self.state.accept_preview(generation, index, result) {
            self.render_state();
        }
    }

    fn handle_applied(
        &mut self,
        generation: u64,
        target_fingerprint: &str,
        result: Result<(), String>,
    ) {
        if !self.state.is_current_apply(generation) {
            return;
        }
        let current_matches = self
            .controller
            .current_track()
            .is_some_and(|track| track.fingerprint() == target_fingerprint);
        if !current_matches {
            self.state.mark_track_changed();
            self.render_state();
            return;
        }
        let succeeded = result.is_ok();
        if !self.state.finish_apply(generation, result) {
            return;
        }
        if succeeded {
            self.controller.reload_lyrics();
        }
        self.render_state();
    }

    fn render_state(&self) {
        self.widgets
            .status
            .set_label(&self.state.status_text(self.language));
        self.widgets
            .preview
            .buffer()
            .set_text(&self.state.preview_text(self.language));
        self.widgets.apply.set_sensitive(self.state.can_apply());
        self.widgets
            .search_button
            .set_sensitive(!self.state.is_searching());
        if self.state.is_searching() {
            self.widgets.spinner.start();
        } else {
            self.widgets.spinner.stop();
        }
    }

    fn clear_results(&self) {
        while let Some(child) = self.widgets.results.first_child() {
            self.widgets.results.remove(&child);
        }
    }
}
