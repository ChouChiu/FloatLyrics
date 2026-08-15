// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Frontend WebKitGTK renderer for the lyrics viewport.

use gtk::prelude::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use webkit6::prelude::*;

use crate::frontend::{AppMsg, UiAction};
use crate::shared::{
    config::AppConfig,
    presentation::{LyricSlotText, LyricsDocument, LyricsFrame},
};

mod bridge;
mod command;
mod metrics;

use bridge::{BridgeState, CommandSlot};
pub(in crate::frontend) use command::{ControlPage, UiSurface};
pub(super) use metrics::{font_family, lyric_content_width};

#[derive(Clone, Default)]
struct Bridge(Rc<RefCell<BridgeState>>);

impl Bridge {
    fn set_ready(&self, ready: bool) {
        self.0.borrow_mut().set_ready(ready);
    }

    fn enqueue(&self, slot: CommandSlot, script: String) {
        self.0.borrow_mut().enqueue(slot, script);
    }

    fn take_pending(&self) -> Option<String> {
        self.0.borrow_mut().take_pending()
    }

    fn complete_dispatch(&self, succeeded: bool) {
        self.0.borrow_mut().complete_dispatch(succeeded);
    }
}

/// A transparent, non-interactive WebKit view backed by packaged HTML.
#[derive(Clone)]
pub(in crate::frontend) struct WebLyricsView {
    web_view: webkit6::WebView,
    bridge: Bridge,
    document_revision: Rc<Cell<Option<u64>>>,
    surface: UiSurface,
    about: Rc<serde_json::Value>,
    available_fonts: Rc<Vec<String>>,
    language: Rc<Cell<floatlyrics_core::i18n::Language>>,
}

impl WebLyricsView {
    pub(in crate::frontend) fn new(
        config: &AppConfig,
        initial_text: &str,
        surface: UiSurface,
        about: Rc<serde_json::Value>,
        sender: relm4::Sender<AppMsg>,
    ) -> Self {
        let settings = webkit6::Settings::new();
        settings.set_auto_load_images(false);
        settings.set_enable_developer_extras(false);
        settings.set_enable_html5_database(false);
        settings.set_enable_html5_local_storage(false);
        settings.set_enable_javascript(true);
        settings.set_enable_media(false);
        settings.set_enable_page_cache(false);
        settings.set_enable_webgl(false);
        settings.set_javascript_can_access_clipboard(false);
        settings.set_javascript_can_open_windows_automatically(false);

        let content_manager = webkit6::UserContentManager::new();
        assert!(
            content_manager.register_script_message_handler("floatLyrics", None),
            "FloatLyrics WebKit message handler must register exactly once"
        );
        content_manager.connect_script_message_received(Some("floatLyrics"), move |_, value| {
            let raw = value.to_str();
            match serde_json::from_str::<UiAction>(&raw) {
                Ok(action) => {
                    let _ = sender.send(AppMsg::UiAction(action));
                }
                Err(error) => tracing::warn!(%error, %raw, "ignored invalid web UI action"),
            }
        });
        let web_view = webkit6::WebView::builder()
            .settings(&settings)
            .user_content_manager(&content_manager)
            .build();
        web_view.set_background_color(&gtk::gdk::RGBA::TRANSPARENT);
        web_view.set_can_target(true);
        web_view.set_focusable(!matches!(surface, UiSurface::Overlay));
        web_view.set_hexpand(true);
        web_view.set_vexpand(true);
        web_view.connect_context_menu(|_, _, _| true);
        let available_fonts = Rc::new(if matches!(surface, UiSurface::FontPicker) {
            let mut families = web_view
                .pango_context()
                .list_families()
                .into_iter()
                .map(|family| family.name().to_string())
                .collect::<Vec<_>>();
            families.sort_by_key(|family| family.to_lowercase());
            families.dedup();
            families
        } else {
            Vec::new()
        });

        let bridge = Bridge::default();

        {
            let bridge = bridge.clone();
            web_view.connect_load_changed(move |view, event| match event {
                webkit6::LoadEvent::Started => bridge.set_ready(false),
                webkit6::LoadEvent::Finished => {
                    bridge.set_ready(true);
                    dispatch_pending(view, &bridge);
                }
                _ => {}
            });
        }

        web_view.load_html(include_str!(concat!(env!("OUT_DIR"), "/lyrics.html")), None);
        let renderer = Self {
            web_view,
            bridge,
            document_revision: Rc::new(Cell::new(None)),
            surface,
            about,
            available_fonts,
            language: Rc::new(Cell::new(config.general.language)),
        };
        renderer.bootstrap(config);
        renderer.apply_config(config);
        renderer.show(LyricsFrame {
            key: "initial".to_string(),
            content: LyricSlotText::message(initial_text),
            position_ms: None,
            playing: false,
            seeking: false,
        });
        renderer
    }

    pub(in crate::frontend) fn bootstrap(&self, config: &AppConfig) {
        self.language.set(config.general.language);
        self.submit(
            CommandSlot::Bootstrap,
            command::bootstrap_script(
                self.surface,
                config,
                config.general.language.catalogue(),
                &self.about,
                &self.available_fonts,
            ),
        );
    }

    pub(in crate::frontend) fn refresh_bootstrap(&self, config: &AppConfig) {
        if self.language.get() != config.general.language {
            self.bootstrap(config);
        }
    }

    pub(in crate::frontend) fn set_config_state(
        &self,
        config: &AppConfig,
        saved: bool,
        error: Option<&str>,
    ) {
        self.submit(
            CommandSlot::ConfigState,
            command::config_state_script(config, saved, error),
        );
    }

    pub(in crate::frontend) fn set_search_state(&self, state: &serde_json::Value) {
        self.submit(
            CommandSlot::SearchState,
            command::search_state_script(state),
        );
    }

    pub(in crate::frontend) fn navigate(&self, page: ControlPage) {
        self.submit(CommandSlot::Navigation, command::navigate_script(page));
    }

    pub(in crate::frontend) fn set_overlay_state(&self, song_info: &str, track_offset: &str) {
        self.submit(
            CommandSlot::OverlayState,
            command::overlay_state_script(song_info, track_offset),
        );
    }

    pub(in crate::frontend) fn set_overlay_placement(&self, classes: Vec<&str>) {
        self.submit(
            CommandSlot::OverlayPlacement,
            command::overlay_placement_script(&classes),
        );
    }

    pub(in crate::frontend) fn set_overlay_appearance(&self, opacity: f64) {
        self.submit(
            CommandSlot::OverlayAppearance,
            command::overlay_appearance_script(opacity),
        );
    }

    pub(in crate::frontend) fn widget(&self) -> webkit6::WebView {
        self.web_view.clone()
    }

    pub(super) fn set_document(&self, document: &LyricsDocument) {
        if self.document_revision.get() == Some(document.revision) {
            return;
        }
        self.document_revision.set(Some(document.revision));
        self.submit(CommandSlot::Document, command::document_script(document));
    }

    pub(super) fn show(&self, frame: LyricsFrame) {
        self.submit(
            CommandSlot::Frame {
                seeking: frame.seeking,
                document_revision: self.document_revision.get(),
            },
            command::frame_script(&frame),
        );
    }

    pub(super) fn apply_config(&self, config: &AppConfig) {
        self.submit(CommandSlot::Config, command::configure_script(config));
    }

    fn submit(&self, slot: CommandSlot, script: serde_json::Result<String>) {
        match script {
            Ok(script) => {
                self.bridge.enqueue(slot, script);
                dispatch_pending(&self.web_view, &self.bridge);
            }
            Err(error) => tracing::warn!(%error, "failed to serialize lyrics for WebKit"),
        }
    }
}

fn dispatch_pending(web_view: &webkit6::WebView, bridge: &Bridge) {
    let Some(script) = bridge.take_pending() else {
        return;
    };

    let weak_view = web_view.downgrade();
    let bridge = bridge.clone();
    web_view.evaluate_javascript(
        &script,
        None,
        Some("floatlyrics://lyrics/update"),
        None::<&gtk::gio::Cancellable>,
        move |result| {
            let succeeded = result.is_ok();
            bridge.complete_dispatch(succeeded);
            if let Err(error) = result {
                tracing::warn!(%error, "failed to update the WebKit lyrics view");
            }
            if succeeded && let Some(view) = weak_view.upgrade() {
                dispatch_pending(&view, &bridge);
            }
        },
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn embedded_html_contains_the_bridge_without_external_assets() {
        let html = include_str!(concat!(env!("OUT_DIR"), "/lyrics.html"));

        assert!(html.contains("Content-Security-Policy"));
        assert!(html.contains("floatLyrics"));
        assert!(!html.contains("<script src="));
        assert!(!html.contains("<link rel=\"stylesheet\""));
    }
}
