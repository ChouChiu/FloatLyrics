// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Messages routed through the Relm4 application model.

use std::{ffi::OsStr, sync::Arc};

use serde::Deserialize;

use crate::shared::{
    config::{AppConfig, WindowPosition},
    presentation::{LyricsDocument, LyricsFrame},
};

use super::{manual_search, settings, view};

#[derive(Debug, Clone)]
pub(super) enum LyricsPresentation {
    Content(Arc<LyricsFrame>),
    Status(floatlyrics_core::i18n::Text),
}

#[derive(Debug)]
pub(in crate::frontend) enum AppMsg {
    Tick,
    /// The application was activated, by a launch or another instance.
    Activated,
    /// The AMLL address settled after an edit.
    ReconnectAmll {
        generation: u64,
    },
    SetSongInfo(String),
    SetTrackOffset(i64),
    SetLyricsDocument(LyricsDocument),
    ShowLyrics(Arc<LyricsFrame>),
    ShowStatus(floatlyrics_core::i18n::Text),
    OpenSettings,
    OpenManualSearch,
    UiAction(UiAction),
    SearchEvent(manual_search::SearchEvent),
    ConfigSaveFinished {
        revision: u64,
        result: settings::ConfigSaveResult,
    },
    WindowMoved(WindowPosition),
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub(in crate::frontend) enum UiAction {
    OpenSettings,
    OpenSearch,
    OpenSettingsPage { page: view::web_lyrics::ControlPage },
    OpenFontPicker,
    CloseFontPicker,
    Quit,
    AdjustTrackOffset { delta_ms: i64 },
    ResetTrackOffset,
    ReloadLyrics,
    SaveConfig { config: Box<AppConfig> },
    SearchLyrics { title: String, artist: String },
    PreviewLyrics { index: usize },
    ApplyLyrics,
    OpenUrl { url: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AppCommand {
    OpenSettings,
    OpenManualSearch,
}

impl AppCommand {
    pub(super) fn from_argument(argument: &OsStr) -> Option<Self> {
        match argument.to_str()? {
            "--settings" => Some(Self::OpenSettings),
            "--select-lyrics" => Some(Self::OpenManualSearch),
            _ => None,
        }
    }

    pub(super) fn message(self) -> AppMsg {
        match self {
            Self::OpenSettings => AppMsg::OpenSettings,
            Self::OpenManualSearch => AppMsg::OpenManualSearch,
        }
    }
}

pub(super) fn requested_commands(
    arguments: &[std::ffi::OsString],
) -> impl Iterator<Item = AppCommand> + '_ {
    arguments
        .iter()
        .filter_map(|argument| AppCommand::from_argument(argument))
}
