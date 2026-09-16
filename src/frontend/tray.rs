// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! StatusNotifierItem system tray icon.
//!
//! The tray mirrors the overlay controls — open the settings window, start a
//! manual lyrics search, reload the lyrics of the current track, and quit — and
//! is the only application entry point that stays reachable in the
//! headline-less AMLL sender run mode.  It deliberately renders no playback
//! controls: media control belongs to the AMLL client that drives them.  Every
//! callback forwards a [`UiAction`] value through [`APP_BROKER`] instead of
//! touching the widget tree, because menu callbacks run on the D-Bus service
//! loop and must not block.

use std::sync::{Arc, RwLock};

use floatlyrics_core::i18n::{Language, Text};
use ksni::{MenuItem, ToolTip, Tray, TrayMethods, menu::StandardItem};

use super::{APP_BROKER, AppMsg, UiAction};

/// Application id shared by the desktop entry, the installed hicolor icon, and
/// the published StatusNotifierItem.
const APP_ID: &str = "io.github.chouchiu.floatlyrics";

/// StatusNotifierItem publishing the FloatLyrics actions.
pub(super) struct FloatLyricsTray {
    /// Language shared with [`TrayHandle`], re-read for every menu rebuild.
    language: Arc<RwLock<Language>>,
}

impl FloatLyricsTray {
    /// Creates a tray that renders its labels in `language`.
    fn new(language: Arc<RwLock<Language>>) -> Self {
        Self { language }
    }

    /// Renders `key` in the currently selected language.
    fn label(&self, key: Text) -> String {
        self.language
            .read()
            .expect("the tray language lock is never held across a panic")
            .text(key)
            .to_string()
    }

    /// Builds the standard menu entry labelled by `key` that runs `activate`.
    fn item(&self, key: Text, activate: fn(&mut Self)) -> MenuItem<Self> {
        MenuItem::from(StandardItem {
            label: self.label(key),
            activate: Box::new(activate),
            ..StandardItem::default()
        })
    }
}

impl Tray for FloatLyricsTray {
    // A left click opens the settings window directly; the menu stays on the
    // secondary (right) click.
    const MENU_ON_ACTIVATE: bool = false;

    fn id(&self) -> String {
        APP_ID.to_string()
    }

    fn title(&self) -> String {
        "FloatLyrics".to_string()
    }

    fn icon_name(&self) -> String {
        APP_ID.to_string()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "FloatLyrics".into(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        publish(UiAction::OpenSettings);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            self.item(Text::OpenSettings, open_settings),
            self.item(Text::SearchLyrics, search_lyrics),
            self.item(Text::ReloadLyrics, reload_lyrics),
            MenuItem::Separator,
            self.item(Text::Quit, quit),
        ]
    }
}

/// Forwards a tray action to the Relm4 application.
fn publish(action: UiAction) {
    APP_BROKER.send(AppMsg::UiAction(action));
}

/// Opens the settings window.
fn open_settings(_tray: &mut FloatLyricsTray) {
    publish(UiAction::OpenSettings);
}

/// Opens manual lyrics search for the currently playing track.
fn search_lyrics(_tray: &mut FloatLyricsTray) {
    publish(UiAction::OpenSearch);
}

/// Re-fetches the lyrics of the currently playing track.
fn reload_lyrics(_tray: &mut FloatLyricsTray) {
    publish(UiAction::ReloadLyrics);
}

/// Quits the application.
fn quit(_tray: &mut FloatLyricsTray) {
    publish(UiAction::Quit);
}

/// Live tray registration together with the language it renders with.
pub(super) struct TrayHandle {
    /// Language shared with the registered [`FloatLyricsTray`].
    language: Arc<RwLock<Language>>,
    /// Registration used to push tray updates to the StatusNotifierHost.
    handle: ksni::Handle<FloatLyricsTray>,
    /// Runtime driving the D-Bus service loop that owns `handle`.
    runtime: tokio::runtime::Handle,
}

impl TrayHandle {
    /// Switches the language used for the tray menu labels.
    ///
    /// The StatusNotifierHost caches the labels it received the last time it
    /// pulled the menu, so updating the shared language alone would leave the
    /// visible menu in the old language.  [`ksni::Handle::update`] makes the
    /// service loop re-read [`Tray::menu`] and emit the changed labels.
    pub(super) fn set_language(&self, language: Language) {
        *self
            .language
            .write()
            .expect("the tray language lock is never held across a panic") = language;
        let handle = self.handle.clone();
        self.runtime.spawn(async move {
            let _ = handle.update(|_| ()).await;
        });
    }
}

/// Publishes the tray icon when it is enabled in `config`.
///
/// Must be called from the GTK thread before the main loop starts, so the
/// registration with the `StatusNotifierWatcher` is driven to completion on
/// `runtime` instead of racing the window setup.  A missing or unusable
/// watcher is not fatal: the application keeps running without a tray icon.
pub(super) fn spawn(
    config: &crate::shared::config::AppConfig,
    runtime: &tokio::runtime::Handle,
) -> Option<TrayHandle> {
    if !config.tray.enabled {
        return None;
    }
    let language = Arc::new(RwLock::new(config.general.language));
    let tray = FloatLyricsTray::new(Arc::clone(&language));
    match runtime.block_on(tray.assume_sni_available(true).spawn()) {
        Ok(handle) => Some(TrayHandle {
            language,
            handle,
            runtime: runtime.clone(),
        }),
        Err(error) => {
            tracing::warn!(%error, "failed to publish the system tray icon");
            None
        }
    }
}

#[cfg(test)]
#[path = "../test/tray_test.rs"]
mod tests;
