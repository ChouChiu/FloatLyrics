// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! React control-center window hosted by GTK/WebKit.

use gtk::prelude::*;
use std::{cell::Cell, rc::Rc};

use crate::shared::config::AppConfig;
use floatlyrics_core::i18n::{Language, Text};

use super::{
    AppMsg,
    view::web_lyrics::{ControlPage, UiSurface, WebLyricsView},
};

#[derive(Clone)]
pub(super) struct ControlCenterView {
    window: gtk::ApplicationWindow,
    web_ui: WebLyricsView,
    language: Rc<Cell<Language>>,
    page: Rc<Cell<ControlPage>>,
}

impl ControlCenterView {
    pub(super) fn new(
        config: &AppConfig,
        about: Rc<serde_json::Value>,
        sender: relm4::Sender<AppMsg>,
    ) -> Self {
        let language = Rc::new(Cell::new(config.general.language));
        let page = Rc::new(Cell::new(ControlPage::General));
        let window = gtk::ApplicationWindow::builder()
            .application(&relm4::main_application())
            .title(window_title(language.get(), page.get()))
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
            UiSurface::ControlCenter,
            about,
            sender,
        );
        window.set_child(Some(&web_ui.widget()));

        Self {
            window,
            web_ui,
            language,
            page,
        }
    }

    pub(super) fn show(&self, page: ControlPage) {
        self.page.set(page);
        self.update_title();
        self.web_ui.navigate(page);
        self.window.present();
    }

    pub(super) fn bootstrap(&self, config: &AppConfig) {
        self.language.set(config.general.language);
        self.update_title();
        self.web_ui.refresh_bootstrap(config);
    }

    pub(super) fn set_config_state(&self, config: &AppConfig, saved: bool, error: Option<&str>) {
        self.web_ui.set_config_state(config, saved, error);
    }

    fn update_title(&self) {
        self.window
            .set_title(Some(window_title(self.language.get(), self.page.get())));
    }
}

fn window_title(language: Language, page: ControlPage) -> &'static str {
    language.text(match page {
        ControlPage::General | ControlPage::Display | ControlPage::Sources => {
            Text::SettingsWindowTitle
        }
        ControlPage::About => Text::AboutWindowTitle,
    })
}
