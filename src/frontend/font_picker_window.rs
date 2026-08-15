// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Standalone React font selection window hosted by GTK/WebKit.

use gtk::prelude::*;
use std::{cell::Cell, rc::Rc};

use crate::shared::config::AppConfig;
use floatlyrics_core::i18n::{Language, Text};

use super::{
    AppMsg,
    view::web_lyrics::{UiSurface, WebLyricsView},
};

#[derive(Clone)]
pub(super) struct FontPickerView {
    window: gtk::ApplicationWindow,
    web_ui: WebLyricsView,
    language: Rc<Cell<Language>>,
}

impl FontPickerView {
    pub(super) fn new(config: &AppConfig, sender: relm4::Sender<AppMsg>) -> Self {
        let language = Rc::new(Cell::new(config.general.language));
        let window = gtk::ApplicationWindow::builder()
            .application(&relm4::main_application())
            .title(language.get().text(Text::FontWindowTitle))
            .default_width(700)
            .default_height(500)
            .resizable(false)
            .decorated(true)
            .modal(true)
            .hide_on_close(true)
            .build();

        let web_ui = WebLyricsView::new(
            config,
            "FloatLyrics",
            UiSurface::FontPicker,
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

    pub(super) fn hide(&self) {
        self.window.set_visible(false);
    }

    pub(super) fn bootstrap(&self, config: &AppConfig) {
        self.language.set(config.general.language);
        self.window
            .set_title(Some(self.language.get().text(Text::FontWindowTitle)));
        self.web_ui.refresh_bootstrap(config);
    }

    pub(super) fn set_config_state(&self, config: &AppConfig, saved: bool, error: Option<&str>) {
        self.web_ui.set_config_state(config, saved, error);
    }
}
