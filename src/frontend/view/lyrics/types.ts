// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

export interface TimedSyllable {
  start_ms: number;
  end_ms: number;
  text: string;
  /** Locally generated reading for this syllable; absent when it has none. */
  romanization?: string;
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
  /** Translation of the background vocal, empty when it has none. */
  background_translation: string;
  /** Where the background vocal is sung within the track. */
  background_start_ms: number;
  /** Exclusive end of the background vocal, when the provider timed one. */
  background_end_ms: number | null;
  /** Words of the background vocal, when the provider timed them. */
  background_syllables: TimedSyllable[];
  /** Vocal part the line belongs to; the AMLL sender writes it as the TTML agent. */
  voice: "primary" | "secondary";
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

export type Language = "en" | "zh-CN" | "zh-TW";
export type LyricsProvider = "qq-music" | "netease" | "kugou" | "lrclib" | "soda-music";
export type AppMode = "floating" | "amll";
export type ChineseRomanizationMode =
  | "auto"
  | "mandarin-pinyin"
  | "cantonese-jyutping"
  | "cantonese-jyutping-no-tones";

export interface AppConfig {
  general: { language: Language; mode: AppMode };
  window: {
    anchor: "bottom-center";
    remember_position: boolean;
    position: { horizontal: number; vertical: number } | null;
    margin: number;
    width: number;
    opacity: number;
    bottom_panel_height: number;
  };
  lyrics: {
    apple_music_style: boolean;
    offset_ms: number;
    provider_order: LyricsProvider[];
    show_translation: boolean;
    show_romanization: boolean;
    chinese_romanization: ChineseRomanizationMode;
    font_order: string[];
    lyric_font_size: number;
    translation_font_size: number;
    romanization_font_size: number;
    played_color: string;
    unplayed_color: string;
    translation_color: string;
    romanization_color: string;
  };
  player: { preferred_players: string[]; ignored_players: string[] };
  amll: { address: string };
  tray: { enabled: boolean };
}

export interface LicenseData {
  dependencies: Array<{ name: string; version: string; license: string }>;
  licenses: Array<{ name: string; id: string; text: string }>;
}

export interface SearchCandidate {
  provider: LyricsProvider;
  title: string;
  artists: string[];
  album: string;
  duration_ms: number | null;
  match_score: number;
}

export interface SearchState {
  title: string;
  artist: string;
  status: string;
  preview: string;
  searching: boolean;
  applying: boolean;
  can_apply: boolean;
  selected_index: number | null;
  candidates: SearchCandidate[];
}

export type ControlPage = "general" | "display" | "integration" | "sources" | "about";
export type SnapClass = "snapped-left" | "snapped-right" | "snapped-top" | "snapped-bottom";

export type LyricsCommand =
  | { type: "configure"; apple_music_style: boolean; style: LyricsStyle }
  | { type: "document"; document: LyricsDocument }
  | { type: "frame"; frame: LyricsFrame }
  | {
      type: "bootstrap";
      surface: "overlay" | "control-center" | "manual-search" | "font-picker";
      config: AppConfig;
      strings: Record<string, string>;
      version: string;
      about: LicenseData;
      available_fonts: string[];
    }
  | { type: "config-state"; config: AppConfig; saved: boolean; error: string | null }
  | { type: "search-state"; state: SearchState }
  | { type: "navigate"; page: ControlPage }
  | { type: "overlay-state"; song_info: string; track_offset: string }
  | { type: "overlay-placement"; classes: SnapClass[] }
  | { type: "overlay-appearance"; opacity: number };

export type UiAction =
  | { type: "open-settings" }
  | { type: "open-search" }
  | { type: "open-settings-page"; page: ControlPage }
  | { type: "open-font-picker" }
  | { type: "close-font-picker" }
  | { type: "quit" }
  | { type: "adjust-track-offset"; delta_ms: number }
  | { type: "reset-track-offset" }
  | { type: "reload-lyrics" }
  | { type: "save-config"; config: AppConfig }
  | { type: "search-lyrics"; title: string; artist: string }
  | { type: "preview-lyrics"; index: number }
  | { type: "apply-lyrics" }
  | { type: "open-url"; url: string };

export interface FloatLyricsBridge {
  dispatch(command: LyricsCommand): void;
}

declare global {
  interface Window {
    floatLyrics?: FloatLyricsBridge;
    floatLyricsPendingCommands?: LyricsCommand[];
    webkit?: {
      messageHandlers?: { floatLyrics?: { postMessage(value: string): void } };
    };
  }
}
