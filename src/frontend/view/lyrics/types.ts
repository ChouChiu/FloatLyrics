// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

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

export interface PresentedLyricLine {
  start_ms: number;
  end_ms: number | null;
  text: string;
  syllables: TimedSyllable[];
  romanization: string;
  translation: string;
  background: string;
}

export interface LyricsDocument {
  revision: number;
  duration_ms: number | null;
  lines: PresentedLyricLine[];
}

export interface LyricsFrame {
  key: string;
  content: LyricContent;
  position_ms: number | null;
  playing: boolean;
  seeking: boolean;
}

export type LyricsCommand =
  | { type: "configure"; apple_music_style: boolean; style: LyricsStyle }
  | { type: "document"; document: LyricsDocument }
  | { type: "frame"; frame: LyricsFrame };

export interface FloatLyricsBridge {
  dispatch(command: LyricsCommand): void;
}

declare global {
  interface Window {
    floatLyrics?: FloatLyricsBridge;
    floatLyricsPendingCommands?: LyricsCommand[];
  }
}
