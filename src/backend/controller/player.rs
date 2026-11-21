// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Applies player watcher events to the controller state and the view.

use std::{sync::Arc, time::Instant};

use floatlyrics_core::{i18n::Text, track::TrackMetadata};

use crate::{
    backend::{
        model::{
            LyricsDisplayState, PLAYBACK_JUMP_THRESHOLD_MS, PlaybackSnapshot,
            apply_position_sample, effective_position_ms, playback_jump_detected,
        },
        mpris::{PlaybackStatus, PlayerLyricsHintEvent, PlayerState, PlayerWatcherEvent},
    },
    shared::presentation::PlayerControl,
};

use super::{
    ControllerContext, ControllerState, LyricsView, PlaybackProjection,
    presentation::{publish_song_info, update_track_display},
};

pub(super) fn handle_player_event(
    event: &PlayerWatcherEvent,
    ctx: &ControllerContext<'_>,
    controller_state: &mut ControllerState,
    playback: &PlaybackProjection,
) {
    match event {
        PlayerWatcherEvent::Connected(state) | PlayerWatcherEvent::Updated(state) => {
            let previous_fingerprint = controller_state
                .latest
                .as_ref()
                .and_then(|snapshot| snapshot.state.track.as_ref())
                .map(TrackMetadata::fingerprint);
            let next_fingerprint = state.track.as_ref().map(TrackMetadata::fingerprint);
            if previous_fingerprint != next_fingerprint {
                controller_state.lyrics_hint = None;
            }
            controller_state.current_fingerprint = next_fingerprint;
            // The first cache load already records a new track. Record only
            // changed metadata for an existing fingerprint on later events.
            if previous_fingerprint == controller_state.current_fingerprint
                && let Some(track) = &state.track
                && controller_state
                    .latest
                    .as_ref()
                    .and_then(|snapshot| snapshot.state.track.as_ref())
                    != Some(track)
            {
                ctx.cache.record_track(track.clone());
            }
            let jump_detected = playback_jump_detected(
                controller_state.latest.as_deref(),
                state.position_ms,
                state,
            );
            if jump_detected {
                controller_state.seek_pending = true;
            }
            controller_state.latest = Some(Arc::new(PlaybackSnapshot::following(
                controller_state.latest.as_deref(),
                state.clone(),
                Instant::now(),
            )));
            playback.set_current_track(state.track.clone());
            publish_player_control(Some(state.control), ctx, controller_state);
            update_player_state(state, ctx, controller_state);
        }
        PlayerWatcherEvent::PositionUpdated {
            track_identity,
            position_ms,
            sampled_at,
        } => {
            if let Some(snapshot) = controller_state.latest.as_mut() {
                let snapshot = Arc::make_mut(snapshot);
                let predicted = effective_position_ms(snapshot);
                if apply_position_sample(
                    snapshot,
                    track_identity.as_deref(),
                    *position_ms,
                    *sampled_at,
                ) && predicted
                    .is_some_and(|value| value.abs_diff(*position_ms) > PLAYBACK_JUMP_THRESHOLD_MS)
                {
                    controller_state.seek_pending = true;
                }
            }
        }
        PlayerWatcherEvent::Disconnected => {
            reset_player_state(controller_state, ctx, playback);
            controller_state.idle_status = Some(Text::OpenPlayer);
            ctx.floating.show_status(Text::OpenPlayer);
        }
        PlayerWatcherEvent::Error(message) => {
            tracing::warn!(%message, "MPRIS listener error");
            reset_player_state(controller_state, ctx, playback);
            controller_state.idle_status = Some(Text::PlayerAttention);
            ctx.floating.show_status(Text::PlayerAttention);
        }
    }
}

/// Forgets the player and tells the view that playback stopped.
///
/// The status shown afterwards is the caller's, because a disconnect and a
/// failed listener are reported differently.
pub(super) fn reset_player_state(
    state: &mut ControllerState,
    ctx: &ControllerContext<'_>,
    playback: &PlaybackProjection,
) {
    state.latest = None;
    state.current_fingerprint = None;
    state.lyrics = LyricsDisplayState::default();
    state.document_dirty = true;
    playback.set_current_track(None);
    state.track_offset_ms = 0;
    state.track_offset_fingerprint = None;
    state.lyrics_hint = None;
    ctx.floating.set_track_offset(0);
    ctx.floating.set_track_metadata(None);
    publish_player_control(None, ctx, state);
    ctx.floating.set_song_info("FloatLyrics");
}

pub(super) fn apply_player_lyrics_hint(event: PlayerLyricsHintEvent, state: &mut ControllerState) {
    let Some(snapshot) = state.latest.as_ref() else {
        return;
    };
    if snapshot.state.bus_name != event.bus_name
        || snapshot
            .state
            .track
            .as_ref()
            .map(TrackMetadata::fingerprint)
            != event.track_fingerprint
        || state.lyrics_hint == event.hint
    {
        return;
    }

    state.lyrics_hint = event.hint;
    state.reload_lyrics();
}

/// Publishes the control state to the view, which only the AMLL sender renders.
///
/// The state is compared with the last published one so a repeated notification
/// does not resend it.
pub(super) fn publish_player_control(
    control: Option<PlayerControl>,
    ctx: &ControllerContext<'_>,
    controller_state: &mut ControllerState,
) {
    if controller_state.player_control == control {
        return;
    }
    controller_state.player_control = control;
    ctx.floating.set_player_control(control.as_ref());
}

pub(super) fn update_player_state(
    state: &PlayerState,
    ctx: &ControllerContext<'_>,
    controller_state: &mut ControllerState,
) {
    // The player's own billing reaches the view here, so a credited
    // restatement has to follow it again.
    controller_state.credited_billing = None;
    let Some(track) = &state.track else {
        ctx.floating.set_track_metadata(None);
        ctx.floating.set_song_info("FloatLyrics");
        ctx.floating.show_status(Text::WaitingForMetadata);
        return;
    };
    ctx.floating.set_track_metadata(Some(track));
    // The song info only changes with the track, so it is published here and not
    // from the per-frame presentation refresh.
    publish_song_info(state, ctx.floating);
    // The snapshot's clock, not the raw sample, so drift held by the snapshot
    // does not rewind this frame either.
    let position_ms = controller_state
        .latest
        .as_deref()
        .and_then(effective_position_ms);
    update_track_display(
        ctx.floating,
        ctx.config,
        &controller_state.lyrics,
        position_ms,
        state.playback_status == PlaybackStatus::Playing,
        controller_state.seek_pending,
        controller_state.track_offset_ms,
    );
}

/// Restates the current track with the artists the lyrics provider credits.
///
/// The provider that supplied the lyrics can name performers the playback source
/// omits — see [`TrackMetadata::artists_including`] — and those names only arrive
/// with the lyrics, so the track is restated once they are known. Nothing is sent
/// when they add nobody, which is the usual case.
///
/// Only the AMLL listener renders track metadata, so the restatement goes
/// through [`LyricsView::set_track_metadata`]; the overlay's song info is left
/// as it is.
pub(super) fn credit_artists(state: &mut ControllerState, view: &dyn LyricsView) {
    let Some(fingerprint) = state.current_fingerprint.as_deref() else {
        return;
    };
    // Only the lyrics of the current track may credit it.
    if state.lyrics.track_fingerprint.as_deref() != Some(fingerprint) {
        return;
    }
    let reconciled = state
        .credited_billing
        .as_ref()
        .is_some_and(|(known, names)| {
            known == fingerprint && names == &state.lyrics.credited_artists
        });
    if reconciled {
        return;
    }
    // Reconciled from here on: without a credited name the view is already
    // right, so the pair is recorded even when nothing is restated.
    state.credited_billing = Some((
        fingerprint.to_string(),
        state.lyrics.credited_artists.clone(),
    ));
    let Some(track) = state
        .latest
        .as_ref()
        .and_then(|snapshot| snapshot.state.track.as_ref())
    else {
        return;
    };
    let Some(artists) = track.artists_including(&state.lyrics.credited_artists) else {
        return;
    };
    let mut credited = track.clone();
    credited.artists = artists;
    view.set_track_metadata(Some(&credited));
}
