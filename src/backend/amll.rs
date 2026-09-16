// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! AMLL WebSocket sender.
//!
//! FloatLyrics is the sending end of the AMLL WebSocket protocol V2
//! (<https://github.com/amll-dev/ws-protocol>): it connects to the address
//! where an AMLL lyrics player listens, announces the current track, and
//! streams the lyrics document and playback progress. The mode is exclusive
//! with the floating overlay because both present the same lyrics, so the
//! frontend selects exactly one of them at startup.
//!
//! Publishing is asynchronous: the lyrics output boundary pushes owned events
//! into an unbounded channel that the connection task drains.

mod client;
mod cover;
mod protocol;
mod ttml;

use std::{cell::RefCell, path::PathBuf, sync::Arc};

use floatlyrics_core::{i18n::Text, track::TrackMetadata};
use tokio::sync::mpsc;

use crate::shared::presentation::{LyricsDocument, LyricsFrame, PlayerControl};

use super::{LyricsView, mpris::MediaCommand};

/// Sending end of the AMLL WebSocket protocol.
///
/// Implements the backend lyrics output boundary, so the controller feeds it
/// exactly like the floating overlay. Dropping the sender stops the connection
/// task.
pub(crate) struct AmllSender {
    events: mpsc::UnboundedSender<AmllEvent>,
    /// Playback requests sent by the listener, taken once by the frontend.
    commands: RefCell<Option<mpsc::UnboundedReceiver<MediaCommand>>>,
    task: tokio::task::JoinHandle<()>,
}

impl AmllSender {
    /// Connects to `address` and streams updates until the sender is dropped.
    pub(crate) fn new(runtime: &tokio::runtime::Handle, address: &str) -> Self {
        let (events, receiver) = mpsc::unbounded_channel();
        let (commands, command_receiver) = mpsc::unbounded_channel();
        let task = runtime.spawn(client::run(address.to_string(), receiver, commands));
        Self {
            events,
            commands: RefCell::new(Some(command_receiver)),
            task,
        }
    }

    /// Takes the playback requests the listener sent, if they were not taken.
    ///
    /// The frontend drains them on its tick and hands them to the MPRIS
    /// control handle, which is the only component that may talk to a player.
    pub(crate) fn take_media_commands(&self) -> Option<mpsc::UnboundedReceiver<MediaCommand>> {
        self.commands.borrow_mut().take()
    }

    fn publish(&self, event: AmllEvent) {
        if let Err(error) = self.events.send(event) {
            tracing::warn!(%error, "AMLL sender stopped accepting updates");
        }
    }
}

impl Drop for AmllSender {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl LyricsView for AmllSender {
    fn set_song_info(&self, _value: &str) {}

    fn set_track_metadata(&self, track: Option<&TrackMetadata>) {
        self.publish(AmllEvent::Music(track.map(music_from_track)));
    }

    fn set_track_offset(&self, _offset_ms: i64) {}

    fn set_player_control(&self, control: Option<&PlayerControl>) {
        self.publish(AmllEvent::Control(control.copied()));
    }

    fn set_lyrics_document(&self, document: LyricsDocument) {
        self.publish(AmllEvent::Lyrics(document));
    }

    /// Publishes the playback clock the listener draws its own progress bar and
    /// line highlighting from.
    fn set_playback(&self, position_ms: Option<u64>, playing: bool) {
        self.publish(AmllEvent::Playback {
            position_ms,
            playing,
        });
    }

    /// The listener renders the document it received against the clock from
    /// [`Self::set_playback`], so a rendered frame carries nothing it needs.
    fn show_lyrics(&self, _frame: Arc<LyricsFrame>) {}

    fn show_status(&self, _key: Text) {}
}

/// Track metadata published to the listener.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Music {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) album: String,
    pub(super) artists: Vec<String>,
    pub(super) duration_ms: u64,
    pub(super) cover: Option<Cover>,
}

/// Where the listener can obtain the album cover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Cover {
    /// A local image streamed over the protocol's binary channel.
    File(PathBuf),
    /// A cover URL forwarded to the listener unchanged.
    Uri(String),
}

/// Updates pushed by the lyrics output boundary.
#[derive(Debug)]
pub(super) enum AmllEvent {
    /// Current track, or `None` when the player disconnected.
    Music(Option<Music>),
    /// Complete lyrics document for the current track.
    Lyrics(LyricsDocument),
    /// Volume, repeat, and shuffle mode of the active player, or `None` when no
    /// player is connected.
    Control(Option<PlayerControl>),
    /// Playback position and play state of the current track.
    Playback {
        position_ms: Option<u64>,
        playing: bool,
    },
}

fn music_from_track(track: &TrackMetadata) -> Music {
    Music {
        id: track.playback_identity(),
        name: track.title.clone(),
        album: track.album.clone().unwrap_or_default(),
        artists: track.artists.clone(),
        duration_ms: track.duration_ms.unwrap_or_default(),
        cover: track.art_url.as_deref().map(cover::cover_source),
    }
}

#[cfg(test)]
#[path = "../test/amll_test.rs"]
mod tests;

#[cfg(test)]
#[path = "../test/amll_integration_test.rs"]
mod integration_tests;
