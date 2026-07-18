// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Frontend WebKitGTK renderer for the lyrics viewport.

use gtk::prelude::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use webkit6::prelude::*;

use crate::shared::{
    config::AppConfig,
    presentation::{LyricSlotText, LyricsDocument, LyricsFrame},
};

mod bridge;
mod command;
mod metrics;

use bridge::{BridgeState, CommandSlot};
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
pub(super) struct WebLyricsView {
    web_view: webkit6::WebView,
    bridge: Bridge,
    document_revision: Rc<Cell<Option<u64>>>,
}

impl WebLyricsView {
    pub(super) fn new(config: &AppConfig, initial_text: &str) -> Self {
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

        let web_view = webkit6::WebView::builder().settings(&settings).build();
        web_view.set_background_color(&gtk::gdk::RGBA::TRANSPARENT);
        web_view.set_can_target(false);
        web_view.set_focusable(false);
        web_view.set_hexpand(true);
        web_view.set_vexpand(true);
        web_view.connect_context_menu(|_, _, _| true);

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
        };
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

    pub(super) fn widget(&self) -> webkit6::WebView {
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
        self.submit(CommandSlot::Frame, command::frame_script(&frame));
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
