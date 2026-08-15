// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Standalone React manual-search window hosted by GTK/WebKit.

use gtk::prelude::*;
use std::{cell::Cell, rc::Rc};

use crate::shared::config::AppConfig;
use floatlyrics_core::i18n::{Language, Text};

use super::{
    AppMsg,
    view::web_lyrics::{UiSurface, WebLyricsView},
};

#[derive(Clone)]
pub(super) struct ManualSearchView {
    window: gtk::ApplicationWindow,
    web_ui: WebLyricsView,
    language: Rc<Cell<Language>>,
}

impl ManualSearchView {
    pub(super) fn new(config: &AppConfig, sender: relm4::Sender<AppMsg>) -> Self {
        let language = Rc::new(Cell::new(config.general.language));
        let window = gtk::ApplicationWindow::builder()
            .application(&relm4::main_application())
            .title(language.get().text(Text::ManualSearchTitle))
            .default_width(980)
            .default_height(680)
            .resizable(true)
            .decorated(true)
            .hide_on_close(true)
            .build();
        window.set_size_request(760, 520);

        let web_ui = WebLyricsView::new(
            config,
            "FloatLyrics",
            UiSurface::ManualSearch,
            Rc::new(serde_json::json!({ "dependencies": [], "licenses": [] })),
            sender,
        );
        window.set_child(Some(&web_ui.widget()));

        Self {
            window,
            web_ui,
            language,
        }
    }

    pub(super) fn show(&self) {
        self.window.present();
    }

    pub(super) fn bootstrap(&self, config: &AppConfig) {
        self.language.set(config.general.language);
        self.window
            .set_title(Some(self.language.get().text(Text::ManualSearchTitle)));
        self.web_ui.refresh_bootstrap(config);
    }

    pub(super) fn set_search_state(&self, state: &serde_json::Value) {
        self.web_ui.set_search_state(state);
    }
}
