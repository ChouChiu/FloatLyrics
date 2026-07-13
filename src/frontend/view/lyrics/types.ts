// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

export interface TimedSyllable {
  start_ms: number;
  end_ms: number;
  text: string;
}

export interface KaraokeContent {
  text: string;
  syllables: TimedSyllable[];
  position_ms: number;
}

export interface LyricContent {
  text: string;
  karaoke: KaraokeContent | null;
  romanization: string;
  translation: string;
}

export interface LyricsStyle {
  font_family: string;
  lyric_font_px: number;
  romanization_font_px: number;
  translation_font_px: number;
  played_color: string;
  unplayed_color: string;
  romanization_color: string;
  translation_color: string;
  transition_ms: number;
}

export interface LyricsPayload {
  key: string;
  content: LyricContent;
  style: LyricsStyle;
}

export interface FloatLyricsBridge {
  render(payload: LyricsPayload): void;
}

declare global {
  interface Window {
    floatLyrics?: FloatLyricsBridge;
    floatLyricsPendingPayload?: LyricsPayload;
  }
}
