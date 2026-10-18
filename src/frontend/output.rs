// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! The lyrics output selected by the run mode, and how a configuration change moves it.

use std::{rc::Rc, time::Duration};

use floatlyrics_core::i18n::I18n;
use gtk::prelude::*;

use crate::{
    backend::{self, mpris::MediaCommand},
    shared::config::{AppConfig, AppMode},
};

use super::{AppMsg, view};

/// Controller pump interval used when no overlay frame clock drives the app.
const TICK_INTERVAL_MS: u64 = 100;

/// The lyrics output selected by `general.mode`, switchable at runtime.
///
/// The overlay is built the first time floating mode is selected and is only
/// hidden afterwards, because a layer-shell surface has to be configured before
/// its window is first shown. The AMLL sender is dropped when another output is
/// selected, which closes its connection.
pub(super) struct LyricsOutput {
    root: gtk::ApplicationWindow,
    pub(super) mode: AppMode,
    pub(super) overlay: Option<view::OverlayView>,
    /// Controller pump installed while the overlay frame clock is not running.
    pump: Option<gtk::glib::SourceId>,
    /// Playback requests sent by the AMLL listener, drained on every tick.
    pub(super) remote_commands: Option<tokio::sync::mpsc::UnboundedReceiver<MediaCommand>>,
}

impl LyricsOutput {
    pub(super) fn new(root: gtk::ApplicationWindow, mode: AppMode) -> Self {
        Self {
            root,
            mode,
            overlay: None,
            pump: None,
            remote_commands: None,
        }
    }

    /// Selects `config.general.mode` and returns the view the controller
    /// should publish to.
    pub(super) fn activate(
        &mut self,
        config: &AppConfig,
        i18n: &I18n,
        backend: &backend::Backend,
        sender: &relm4::Sender<AppMsg>,
    ) -> Rc<dyn backend::LyricsView> {
        self.mode = config.general.mode;
        match self.mode {
            AppMode::Floating => {
                self.remote_commands = None;
                if self.overlay.is_none() {
                    let overlay = view::build(
                        &self.root,
                        config,
                        i18n.clone(),
                        Rc::new(serde_json::json!({ "dependencies": [], "licenses": [] })),
                        sender.clone(),
                    );
                    // The frame clock only runs while the overlay is mapped, so
                    // it stops by itself while another output is selected.
                    let input = sender.clone();
                    overlay.tick_widget().add_tick_callback(move |_, _| {
                        let _ = input.send(AppMsg::Tick);
                        gtk::glib::ControlFlow::Continue
                    });
                    self.overlay = Some(overlay);
                }
                if let Some(pump) = self.pump.take() {
                    pump.remove();
                }
                Rc::new(view::OverlaySender::new(sender.clone()))
            }
            AppMode::Amll => {
                self.root.set_visible(false);
                let amll_sender = backend.amll_sender(&config.amll);
                self.remote_commands = amll_sender.take_media_commands();
                if self.pump.is_none() {
                    // The overlay provides the frame clock; without it the sender
                    // mode needs its own pump for MPRIS events and progress.
                    let input = sender.clone();
                    self.pump = Some(gtk::glib::timeout_add_local(
                        Duration::from_millis(TICK_INTERVAL_MS),
                        move || {
                            let _ = input.send(AppMsg::Tick);
                            gtk::glib::ControlFlow::Continue
                        },
                    ));
                }
                amll_sender
            }
        }
    }

    /// Shows the overlay when it is the selected output.
    pub(super) fn present(&self) {
        if self.mode == AppMode::Floating {
            self.root.present();
        }
    }

    /// The overlay, while it is the selected output.
    pub(super) fn overlay(&self) -> Option<&view::OverlayView> {
        self.overlay
            .as_ref()
            .filter(|_| self.mode == AppMode::Floating)
    }
}

/// How the lyrics output changes between two configurations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OutputChange {
    None,
    /// Another run mode is selected.
    Mode,
    /// The AMLL sender stays the output but connects elsewhere.
    AmllAddress,
}

pub(super) fn output_change(current: &AppConfig, next: &AppConfig) -> OutputChange {
    if current.general.mode != next.general.mode {
        OutputChange::Mode
    } else if next.general.mode == AppMode::Amll && current.amll.address != next.amll.address {
        OutputChange::AmllAddress
    } else {
        OutputChange::None
    }
}

pub(super) fn should_reload_lyrics(current: &AppConfig, next: &AppConfig) -> bool {
    current.lyrics.provider_order != next.lyrics.provider_order
        || (!current.lyrics.show_translation && next.lyrics.show_translation)
        || (!current.lyrics.show_romanization && next.lyrics.show_romanization)
        || (next.lyrics.show_romanization
            && current.lyrics.chinese_romanization != next.lyrics.chinese_romanization)
}
