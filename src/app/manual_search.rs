// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

//! Track-specific manual lyrics search and selection window.

use gtk::prelude::*;
use std::{cell::Cell, cell::RefCell, rc::Rc, sync::mpsc, time::Duration};

use crate::{
    cache::LyricsCache,
    i18n::{I18n, Language, Text},
    lyrics::{
        FetchedLyrics, LyricsCandidate, LyricsProvider, fetch_candidate_lyrics,
        search_lyrics_candidates,
    },
    track::TrackMetadata,
};

use super::{
    controller::ControllerHandle,
    localization::{bind_button_label, bind_entry_placeholder, bind_label, bind_window_title},
};

const WINDOW_WIDTH: i32 = 820;
const WINDOW_HEIGHT: i32 = 560;

#[derive(Debug)]
enum SearchEvent {
    Candidates {
        generation: u64,
        result: Result<Vec<LyricsCandidate>, String>,
    },
    Preview {
        generation: u64,
        index: usize,
        result: Result<Option<FetchedLyrics>, String>,
    },
}

#[derive(Default)]
struct SearchState {
    generation: u64,
    target_track: Option<TrackMetadata>,
    candidates: Vec<LyricsCandidate>,
    preview_index: Option<usize>,
    selected: Option<(usize, FetchedLyrics)>,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

#[derive(Clone)]
pub(super) struct ManualSearchWindow {
    window: gtk::ApplicationWindow,
    title: gtk::Entry,
    artist: gtk::Entry,
    start_search: Rc<dyn Fn()>,
    controller: ControllerHandle,
}

impl ManualSearchWindow {
    pub(super) fn new(
        app: &gtk::Application,
        runtime: tokio::runtime::Handle,
        cache: Rc<dyn LyricsCache>,
        controller: ControllerHandle,
        i18n: I18n,
    ) -> Self {
        let title = gtk::Entry::builder().hexpand(true).build();
        bind_entry_placeholder(&title, &i18n, Text::SongTitle);
        let artist = gtk::Entry::builder().hexpand(true).build();
        bind_entry_placeholder(&artist, &i18n, Text::Artist);
        let search_button = gtk::Button::builder()
            .css_classes(["suggested-action"])
            .build();
        bind_button_label(&search_button, &i18n, Text::Search);
        let spinner = gtk::Spinner::new();

        let search_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        search_bar.add_css_class("manual-search-bar");
        let title_label = gtk::Label::new(None);
        bind_label(&title_label, &i18n, Text::Title);
        search_bar.append(&title_label);
        search_bar.append(&title);
        let artist_label = gtk::Label::new(None);
        bind_label(&artist_label, &i18n, Text::Artist);
        search_bar.append(&artist_label);
        search_bar.append(&artist);
        search_bar.append(&spinner);
        search_bar.append(&search_button);

        let results = gtk::ListBox::new();
        results.set_selection_mode(gtk::SelectionMode::Single);
        results.add_css_class("boxed-list");
        let results_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .min_content_width(340)
            .child(&results)
            .build();

        let preview = gtk::TextView::builder()
            .editable(false)
            .cursor_visible(false)
            .monospace(true)
            .wrap_mode(gtk::WrapMode::WordChar)
            .left_margin(12)
            .right_margin(12)
            .top_margin(12)
            .bottom_margin(12)
            .build();
        preview.add_css_class("manual-preview");
        let preview_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hexpand(true)
            .child(&preview)
            .build();

        let paned = gtk::Paned::builder()
            .orientation(gtk::Orientation::Horizontal)
            .start_child(&results_scroll)
            .end_child(&preview_scroll)
            .position(360)
            .wide_handle(true)
            .vexpand(true)
            .build();

        let status = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .hexpand(true)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        let apply = gtk::Button::builder()
            .sensitive(false)
            .css_classes(["suggested-action"])
            .build();
        bind_button_label(&apply, &i18n, Text::ApplySelectedLyrics);
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        footer.add_css_class("manual-search-footer");
        footer.append(&status);
        footer.append(&apply);

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&search_bar);
        root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        root.append(&paned);
        root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        root.append(&footer);

        let header_title = gtk::Label::new(None);
        bind_label(&header_title, &i18n, Text::ManualSearchTitle);
        let header = gtk::HeaderBar::builder()
            .title_widget(&header_title)
            .show_title_buttons(true)
            .build();
        let handle = gtk::WindowHandle::new();
        handle.set_child(Some(&header));
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .default_width(WINDOW_WIDTH)
            .default_height(WINDOW_HEIGHT)
            .resizable(false)
            .titlebar(&handle)
            .child(&root)
            .hide_on_close(true)
            .build();
        bind_window_title(&window, &i18n, Text::ManualSearchTitle);

        let state = Rc::new(RefCell::new(SearchState::default()));
        let status_state = Rc::new(RefCell::new(ManualStatus::Text(Text::SearchAfterPlayback)));
        let preview_state = Rc::new(RefCell::new(PreviewState::Text(
            Text::SelectCandidatePreview,
        )));
        let set_status: Rc<dyn Fn(ManualStatus)> = {
            let status = status.clone();
            let status_state = Rc::clone(&status_state);
            let i18n = i18n.clone();
            Rc::new(move |next| {
                *status_state.borrow_mut() = next;
                status.set_label(&status_state.borrow().render(i18n.language()));
            })
        };
        let set_preview: Rc<dyn Fn(PreviewState)> = {
            let preview = preview.clone();
            let preview_state = Rc::clone(&preview_state);
            let i18n = i18n.clone();
            Rc::new(move |next| {
                *preview_state.borrow_mut() = next;
                preview
                    .buffer()
                    .set_text(&preview_state.borrow().render(i18n.language()));
            })
        };
        {
            let status = status.clone();
            let preview = preview.clone();
            let status_state = Rc::clone(&status_state);
            let preview_state = Rc::clone(&preview_state);
            i18n.subscribe(move |language| {
                status.set_label(&status_state.borrow().render(language));
                preview
                    .buffer()
                    .set_text(&preview_state.borrow().render(language));
            });
        }
        let (sender, receiver) = mpsc::channel::<SearchEvent>();
        let rebuilding_results = Rc::new(Cell::new(false));

        {
            let state = Rc::clone(&state);
            let runtime = runtime.clone();
            let sender = sender.clone();
            let apply = apply.clone();
            let set_status = Rc::clone(&set_status);
            let set_preview = Rc::clone(&set_preview);
            let rebuilding_results = Rc::clone(&rebuilding_results);
            results.connect_row_selected(move |_, row| {
                if rebuilding_results.get() {
                    return;
                }
                let Some(index) = row.and_then(|row| usize::try_from(row.index()).ok()) else {
                    return;
                };
                let (generation, candidate) = {
                    let mut state = state.borrow_mut();
                    state.selected = None;
                    state.preview_index = Some(index);
                    let Some(candidate) = state.candidates.get(index).cloned() else {
                        return;
                    };
                    (state.generation, candidate)
                };
                apply.set_sensitive(false);
                set_status(ManualStatus::Text(Text::LoadingPreview));
                set_preview(PreviewState::Text(Text::LoadingPreview));
                let sender = sender.clone();
                runtime.spawn(async move {
                    let result = fetch_candidate_lyrics(&candidate)
                        .await
                        .map_err(|error| error.to_string());
                    let _ = sender.send(SearchEvent::Preview {
                        generation,
                        index,
                        result,
                    });
                });
            });
        }

        let start_search: Rc<dyn Fn()> = {
            let state = Rc::clone(&state);
            let runtime = runtime.clone();
            let sender = sender.clone();
            let title = title.clone();
            let artist = artist.clone();
            let controller = controller.clone();
            let results = results.clone();
            let apply = apply.clone();
            let spinner = spinner.clone();
            let search_button = search_button.clone();
            let set_status = Rc::clone(&set_status);
            let set_preview = Rc::clone(&set_preview);
            Rc::new(move || {
                let Some(target_track) = controller.current_track() else {
                    set_status(ManualStatus::Text(Text::NoTrackPlaying));
                    return;
                };
                let search_title = title.text().trim().to_string();
                if search_title.is_empty() {
                    set_status(ManualStatus::Text(Text::EnterSongTitle));
                    return;
                }
                let artists = artist
                    .text()
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                let search_track = TrackMetadata {
                    title: search_title,
                    artists,
                    album: None,
                    duration_ms: target_track.duration_ms,
                    mpris_track_id: None,
                };
                let generation = {
                    let mut state = state.borrow_mut();
                    state.generation = state.generation.wrapping_add(1);
                    state.target_track = Some(target_track);
                    state.candidates.clear();
                    state.preview_index = None;
                    state.selected = None;
                    state.generation
                };
                while let Some(child) = results.first_child() {
                    results.remove(&child);
                }
                apply.set_sensitive(false);
                set_preview(PreviewState::Text(Text::SearchingCandidates));
                set_status(ManualStatus::Text(Text::SearchingProviders));
                search_button.set_sensitive(false);
                spinner.start();
                let sender = sender.clone();
                runtime.spawn(async move {
                    let providers = [LyricsProvider::QqMusic, LyricsProvider::NetEase];
                    let result = search_lyrics_candidates(&search_track, &providers)
                        .await
                        .map_err(|error| error.to_string());
                    let _ = sender.send(SearchEvent::Candidates { generation, result });
                });
            })
        };

        {
            let start_search = Rc::clone(&start_search);
            search_button.connect_clicked(move |_| start_search());
        }
        {
            let start_search = Rc::clone(&start_search);
            title.connect_activate(move |_| start_search());
        }
        {
            let start_search = Rc::clone(&start_search);
            artist.connect_activate(move |_| start_search());
        }

        {
            let state = Rc::clone(&state);
            let cache = Rc::clone(&cache);
            let controller = controller.clone();
            let set_status = Rc::clone(&set_status);
            apply.connect_clicked(move |_| {
                let state = state.borrow();
                let Some(target) = state.target_track.as_ref() else {
                    return;
                };
                let Some((_, fetched)) = state.selected.as_ref() else {
                    return;
                };
                if controller
                    .current_track()
                    .as_ref()
                    .map(TrackMetadata::fingerprint)
                    != Some(target.fingerprint())
                {
                    set_status(ManualStatus::Text(Text::TrackChanged));
                    return;
                }
                let result = cache
                    .insert_lyrics(
                        fetched.provider,
                        fetched.provider_track_id.as_deref(),
                        &fetched.title,
                        &fetched.artists,
                        &fetched.raw_lyrics,
                    )
                    .and_then(|lyrics_id| {
                        cache.bind_manual_match(&target.fingerprint(), lyrics_id)
                    });
                match result {
                    Ok(()) => {
                        controller.reload_lyrics();
                        set_status(ManualStatus::Text(Text::LyricsApplied));
                    }
                    Err(error) => {
                        set_status(ManualStatus::Detail(Text::ApplyFailed, error.to_string()))
                    }
                }
            });
        }

        {
            let state = Rc::clone(&state);
            let results = results.clone();
            let spinner = spinner.clone();
            let apply = apply.clone();
            let search_button = search_button.clone();
            let i18n = i18n.clone();
            let set_status = Rc::clone(&set_status);
            let set_preview = Rc::clone(&set_preview);
            gtk::glib::timeout_add_local(Duration::from_millis(50), move || {
                for event in receiver.try_iter() {
                    match event {
                        SearchEvent::Candidates { generation, result } => {
                            if state.borrow().generation != generation {
                                continue;
                            }
                            spinner.stop();
                            search_button.set_sensitive(true);
                            match result {
                                Ok(candidates) => {
                                    let count = candidates.len();
                                    state.borrow_mut().candidates = candidates.clone();
                                    for candidate in &candidates {
                                        results.append(&candidate_row(candidate, i18n.language()));
                                    }
                                    if count == 0 {
                                        set_status(ManualStatus::Text(Text::NoCandidates));
                                        set_preview(PreviewState::Text(Text::NoCandidates));
                                    } else {
                                        set_status(ManualStatus::CandidatesFound(count));
                                        if let Some(row) = results.row_at_index(0) {
                                            results.select_row(Some(&row));
                                        }
                                    }
                                }
                                Err(error) => {
                                    set_status(ManualStatus::Detail(Text::SearchFailed, error));
                                    set_preview(PreviewState::Text(
                                        Text::LyricsSearchPreviewFailed,
                                    ));
                                }
                            }
                        }
                        SearchEvent::Preview {
                            generation,
                            index,
                            result,
                        } => {
                            let is_current = {
                                let state = state.borrow();
                                state.generation == generation && state.preview_index == Some(index)
                            };
                            if !is_current {
                                continue;
                            }
                            match result {
                                Ok(Some(fetched)) => {
                                    set_preview(PreviewState::Lyrics(fetched.raw_lyrics.clone()));
                                    state.borrow_mut().selected = Some((index, fetched));
                                    apply.set_sensitive(true);
                                    set_status(ManualStatus::Text(Text::PreviewReady));
                                }
                                Ok(None) => {
                                    set_preview(PreviewState::Text(Text::CandidateUnavailable));
                                    set_status(ManualStatus::Text(Text::CandidateUnavailable));
                                }
                                Err(error) => {
                                    set_preview(PreviewState::Text(Text::PreviewLoadFailed));
                                    set_status(ManualStatus::Detail(Text::LoadingFailed, error));
                                }
                            }
                        }
                    }
                }
                gtk::glib::ControlFlow::Continue
            });
        }

        {
            let state = Rc::clone(&state);
            let results = results.clone();
            let rebuilding_results = Rc::clone(&rebuilding_results);
            i18n.subscribe(move |language| {
                let (candidates, selected) = {
                    let state = state.borrow();
                    (state.candidates.clone(), state.preview_index)
                };
                rebuilding_results.set(true);
                while let Some(child) = results.first_child() {
                    results.remove(&child);
                }
                for candidate in &candidates {
                    results.append(&candidate_row(candidate, language));
                }
                if let Some(index) = selected.and_then(|index| i32::try_from(index).ok())
                    && let Some(row) = results.row_at_index(index)
                {
                    results.select_row(Some(&row));
                }
                rebuilding_results.set(false);
            });
        }

        install_css();
        Self {
            window,
            title,
            artist,
            start_search,
            controller,
        }
    }

    pub(super) fn present(&self) {
        if let Some(track) = self.controller.current_track() {
            self.title.set_text(&track.title);
            self.artist.set_text(&track.display_artist());
        }
        self.window.present();
        (self.start_search)();
    }
}

fn candidate_row(candidate: &LyricsCandidate, language: Language) -> gtk::ListBoxRow {
    let title = gtk::Label::builder()
        .label(&candidate.title)
        .halign(gtk::Align::Start)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["heading"])
        .build();
    let detail = gtk::Label::builder()
        .label(format!(
            "{}  ·  {}  ·  {}",
            candidate.artists.join(", "),
            provider_name(candidate.provider, language),
            duration_text(candidate.duration_ms)
        ))
        .halign(gtk::Align::Start)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["dim-label"])
        .build();
    let labels = gtk::Box::new(gtk::Orientation::Vertical, 3);
    labels.set_hexpand(true);
    labels.append(&title);
    labels.append(&detail);
    let score = gtk::Label::builder()
        .label(format!("{}%", candidate.match_score.max(0)))
        .valign(gtk::Align::Center)
        .css_classes(["dim-label"])
        .build();
    score.set_tooltip_text(Some(language.text(Text::MatchScore)));
    let row_content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row_content.add_css_class("manual-result-row");
    row_content.append(&labels);
    row_content.append(&score);
    gtk::ListBoxRow::builder().child(&row_content).build()
}

fn provider_name(provider: LyricsProvider, language: Language) -> &'static str {
    match (provider, language) {
        (LyricsProvider::QqMusic, Language::English) => "QQ Music",
        (LyricsProvider::NetEase, Language::English) => "NetEase Cloud Music",
        (LyricsProvider::QqMusic, _) => "QQ 音乐",
        (LyricsProvider::NetEase, Language::SimplifiedChinese) => "网易云音乐",
        (LyricsProvider::NetEase, Language::TraditionalChinese) => "網易雲音樂",
        (LyricsProvider::LrcLib, _) => "LRCLIB",
    }
}

fn duration_text(duration_ms: Option<i32>) -> String {
    let seconds = duration_ms.unwrap_or_default().max(0) / 1_000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

fn install_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        r#"
        .manual-search-bar, .manual-search-footer { padding: 12px; }
        .manual-result-row { padding: 10px 12px; }
        "#,
    );
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

#[cfg(test)]
#[path = "../test/manual_search_test.rs"]
mod tests;
