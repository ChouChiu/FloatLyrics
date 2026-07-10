//! Track-specific manual lyrics search and selection window.

use gtk::prelude::*;
use std::{cell::RefCell, rc::Rc, sync::mpsc, time::Duration};

use crate::{
    cache::Cache,
    lyrics::{
        FetchedLyrics, LyricsCandidate, LyricsProvider, fetch_candidate_lyrics,
        search_lyrics_candidates,
    },
    track::TrackMetadata,
};

use super::controller::ControllerHandle;

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
        cache: Rc<Cache>,
        controller: ControllerHandle,
    ) -> Self {
        let title = gtk::Entry::builder()
            .hexpand(true)
            .placeholder_text("歌曲名")
            .build();
        let artist = gtk::Entry::builder()
            .hexpand(true)
            .placeholder_text("艺术家")
            .build();
        let search_button = gtk::Button::builder()
            .label("搜索")
            .css_classes(["suggested-action"])
            .build();
        let spinner = gtk::Spinner::new();

        let search_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        search_bar.add_css_class("manual-search-bar");
        search_bar.append(&gtk::Label::new(Some("标题")));
        search_bar.append(&title);
        search_bar.append(&gtk::Label::new(Some("艺术家")));
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
        preview.buffer().set_text("选择候选歌词后将在这里预览");
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
            .label("播放歌曲后可搜索并手动绑定歌词")
            .halign(gtk::Align::Start)
            .hexpand(true)
            .css_classes(["dim-label"])
            .build();
        let apply = gtk::Button::builder()
            .label("应用所选歌词")
            .sensitive(false)
            .css_classes(["suggested-action"])
            .build();
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

        let header = gtk::HeaderBar::builder()
            .title_widget(&gtk::Label::new(Some("手动选择歌词")))
            .show_title_buttons(true)
            .build();
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("FloatLyrics 手动选择歌词")
            .default_width(WINDOW_WIDTH)
            .default_height(WINDOW_HEIGHT)
            .titlebar(&header)
            .child(&root)
            .hide_on_close(true)
            .build();

        let state = Rc::new(RefCell::new(SearchState::default()));
        let (sender, receiver) = mpsc::channel::<SearchEvent>();

        {
            let state = Rc::clone(&state);
            let runtime = runtime.clone();
            let sender = sender.clone();
            let preview = preview.clone();
            let status = status.clone();
            let apply = apply.clone();
            results.connect_row_selected(move |_, row| {
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
                status.set_label("正在加载歌词预览…");
                preview.buffer().set_text("正在加载歌词预览…");
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
            let status = status.clone();
            let preview = preview.clone();
            let results = results.clone();
            let apply = apply.clone();
            let spinner = spinner.clone();
            Rc::new(move || {
                let Some(target_track) = controller.current_track() else {
                    status.set_label("当前没有正在播放的歌曲");
                    return;
                };
                let search_title = title.text().trim().to_string();
                if search_title.is_empty() {
                    status.set_label("请输入歌曲标题");
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
                preview.buffer().set_text("正在搜索候选歌词…");
                status.set_label("正在搜索 QQ 音乐和网易云音乐…");
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
            let status = status.clone();
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
                    status.set_label("歌曲已经切换，请重新搜索");
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
                        status.set_label("已应用并记住这条歌词");
                    }
                    Err(error) => status.set_label(&format!("应用失败：{error}")),
                }
            });
        }

        {
            let state = Rc::clone(&state);
            let results = results.clone();
            let preview = preview.clone();
            let status = status.clone();
            let spinner = spinner.clone();
            let apply = apply.clone();
            gtk::glib::timeout_add_local(Duration::from_millis(50), move || {
                for event in receiver.try_iter() {
                    match event {
                        SearchEvent::Candidates { generation, result } => {
                            if state.borrow().generation != generation {
                                continue;
                            }
                            spinner.stop();
                            match result {
                                Ok(candidates) => {
                                    let count = candidates.len();
                                    state.borrow_mut().candidates = candidates.clone();
                                    for candidate in &candidates {
                                        results.append(&candidate_row(candidate));
                                    }
                                    if count == 0 {
                                        status.set_label("没有找到候选歌词");
                                        preview.buffer().set_text("没有找到候选歌词");
                                    } else {
                                        status.set_label(&format!("找到 {count} 条候选歌词"));
                                        if let Some(row) = results.row_at_index(0) {
                                            results.select_row(Some(&row));
                                        }
                                    }
                                }
                                Err(error) => {
                                    status.set_label(&format!("搜索失败：{error}"));
                                    preview.buffer().set_text("搜索歌词失败");
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
                                    preview.buffer().set_text(&fetched.raw_lyrics);
                                    state.borrow_mut().selected = Some((index, fetched));
                                    apply.set_sensitive(true);
                                    status.set_label("预览已加载，可应用所选歌词");
                                }
                                Ok(None) => {
                                    preview.buffer().set_text("该候选没有可用歌词");
                                    status.set_label("该候选没有可用歌词");
                                }
                                Err(error) => {
                                    preview.buffer().set_text("加载歌词预览失败");
                                    status.set_label(&format!("加载失败：{error}"));
                                }
                            }
                        }
                    }
                }
                gtk::glib::ControlFlow::Continue
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

fn candidate_row(candidate: &LyricsCandidate) -> gtk::ListBoxRow {
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
            provider_name(candidate.provider),
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
    let row_content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row_content.add_css_class("manual-result-row");
    row_content.append(&labels);
    row_content.append(&score);
    gtk::ListBoxRow::builder().child(&row_content).build()
}

fn provider_name(provider: LyricsProvider) -> &'static str {
    match provider {
        LyricsProvider::QqMusic => "QQ 音乐",
        LyricsProvider::NetEase => "网易云音乐",
        LyricsProvider::LrcLib => "LRCLIB",
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
mod tests {
    use super::*;

    #[test]
    fn formats_candidate_duration() {
        assert_eq!(duration_text(Some(185_000)), "3:05");
        assert_eq!(duration_text(None), "0:00");
    }
}
