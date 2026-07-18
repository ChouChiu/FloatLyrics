// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

//! Pure mutable presentation state for the overlay adapter.

use std::{cell::RefCell, rc::Rc};

use floatlyrics_core::i18n::Text;

use crate::shared::{config::AppConfig, presentation::LyricsFrame};

use super::layout::lyrics_resize_animation;

#[derive(Debug, Clone, PartialEq, Eq)]
struct LyricsLayoutKey {
    frame_key: String,
    romanization: String,
    translation: String,
}

impl From<&LyricsFrame> for LyricsLayoutKey {
    fn from(frame: &LyricsFrame) -> Self {
        Self {
            frame_key: frame.key.clone(),
            romanization: frame.content.romanization.clone(),
            translation: frame.content.translation.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OverlayMetrics {
    pub(super) compact_width: i32,
    pub(super) lyric_font_size: i32,
    pub(super) romanization_font_size: i32,
    pub(super) translation_font_size: i32,
    pub(super) apple_music_style: bool,
}

#[derive(Debug)]
pub(super) struct OverlayState {
    metrics: OverlayMetrics,
    static_status: Option<Text>,
    last_lyrics_layout: Option<LyricsLayoutKey>,
    animation_generation: u64,
}

/// Shared adapter state used by GTK callbacks.
#[derive(Clone)]
pub(super) struct OverlayStateHandle(Rc<RefCell<OverlayState>>);

impl OverlayStateHandle {
    pub(super) fn new(config: &AppConfig, compact_width: i32) -> Self {
        Self(Rc::new(RefCell::new(OverlayState::new(
            config,
            compact_width,
        ))))
    }

    pub(super) fn apply_config(&self, config: &AppConfig, compact_width: i32) {
        self.0.borrow_mut().apply_config(config, compact_width);
    }

    pub(super) fn show_content(&self) {
        self.0.borrow_mut().show_content();
    }

    pub(super) fn show_status(&self, key: Text) {
        self.0.borrow_mut().show_status(key);
    }

    pub(super) fn static_status(&self) -> Option<Text> {
        self.0.borrow().static_status()
    }

    pub(super) fn register_frame(&self, frame: &LyricsFrame) -> Option<bool> {
        self.0.borrow_mut().register_frame(frame)
    }

    pub(super) fn metrics(&self) -> OverlayMetrics {
        self.0.borrow().metrics()
    }

    pub(super) fn animation_generation(&self) -> u64 {
        self.0.borrow().animation_generation()
    }

    pub(super) fn cancel_animation(&self) -> u64 {
        self.0.borrow_mut().cancel_animation()
    }
}

impl OverlayState {
    pub(super) fn new(config: &AppConfig, compact_width: i32) -> Self {
        Self {
            metrics: OverlayMetrics {
                compact_width,
                lyric_font_size: config.lyrics.lyric_font_size,
                romanization_font_size: config.lyrics.romanization_font_size,
                translation_font_size: config.lyrics.translation_font_size,
                apple_music_style: config.lyrics.apple_music_style,
            },
            static_status: Some(Text::OpenSpotify),
            last_lyrics_layout: None,
            animation_generation: 0,
        }
    }

    pub(super) fn apply_config(&mut self, config: &AppConfig, compact_width: i32) {
        self.metrics = OverlayMetrics {
            compact_width,
            lyric_font_size: config.lyrics.lyric_font_size,
            romanization_font_size: config.lyrics.romanization_font_size,
            translation_font_size: config.lyrics.translation_font_size,
            apple_music_style: config.lyrics.apple_music_style,
        };
        self.last_lyrics_layout = None;
        self.cancel_animation();
    }

    pub(super) fn show_content(&mut self) {
        self.static_status = None;
    }

    pub(super) fn show_status(&mut self, key: Text) {
        self.static_status = Some(key);
    }

    pub(super) fn static_status(&self) -> Option<Text> {
        self.static_status
    }

    pub(super) fn register_frame(&mut self, frame: &LyricsFrame) -> Option<bool> {
        let layout_key = LyricsLayoutKey::from(frame);
        let layout_changed = self.last_lyrics_layout.as_ref() != Some(&layout_key);
        let resize = lyrics_resize_animation(self.metrics.apple_music_style, layout_changed);
        if resize.is_some() {
            self.last_lyrics_layout = Some(layout_key);
        }
        resize
    }

    pub(super) fn metrics(&self) -> OverlayMetrics {
        self.metrics
    }

    pub(super) fn animation_generation(&self) -> u64 {
        self.animation_generation
    }

    pub(super) fn cancel_animation(&mut self) -> u64 {
        self.animation_generation = self.animation_generation.wrapping_add(1);
        self.animation_generation
    }
}

#[cfg(test)]
#[path = "../../test/overlay_state_test.rs"]
mod tests;
