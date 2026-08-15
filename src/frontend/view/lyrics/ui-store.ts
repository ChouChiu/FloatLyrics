// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import type {
  AppConfig,
  ControlPage,
  LicenseData,
  LyricsCommand,
  SearchState,
  SnapClass,
  UiAction,
} from "./types";

export interface UiState {
  surface: "overlay" | "control-center" | "manual-search" | "font-picker" | null;
  config: AppConfig | null;
  strings: Record<string, string>;
  version: string;
  about: LicenseData;
  availableFonts: string[];
  page: ControlPage;
  saved: boolean;
  saveError: string | null;
  search: SearchState | null;
  songInfo: string;
  trackOffset: string;
  snapClasses: SnapClass[];
  panelOpacity: number;
}

const initialState: UiState = {
  surface: null,
  config: null,
  strings: {},
  version: "",
  about: { dependencies: [], licenses: [] },
  availableFonts: [],
  page: "general",
  saved: false,
  saveError: null,
  search: null,
  songInfo: "FloatLyrics",
  trackOffset: "0 ms",
  snapClasses: [],
  panelOpacity: 0.78,
};

export function advanceUiState(state: UiState, command: LyricsCommand): UiState {
  switch (command.type) {
    case "bootstrap":
      return {
        ...state,
        surface: command.surface,
        config: command.config,
        strings: command.strings,
        version: command.version,
        about: command.about,
        availableFonts: command.available_fonts,
        panelOpacity: command.config.window.opacity,
      };
    case "config-state":
      return {
        ...state,
        config: command.config,
        saved: command.saved,
        saveError: command.error,
      };
    case "search-state":
      return { ...state, search: command.state };
    case "navigate":
      return { ...state, page: command.page };
    case "overlay-state":
      return { ...state, songInfo: command.song_info, trackOffset: command.track_offset };
    case "overlay-placement":
      return { ...state, snapClasses: command.classes };
    case "overlay-appearance":
      return { ...state, panelOpacity: command.opacity };
    default:
      return state;
  }
}

type Listener = () => void;

class UiStore {
  private state = initialState;
  private readonly listeners = new Set<Listener>();

  readonly getSnapshot = (): UiState => this.state;

  readonly subscribe = (listener: Listener): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  readonly dispatch = (command: LyricsCommand): void => {
    const next = advanceUiState(this.state, command);
    if (next === this.state) return;
    this.state = next;
    for (const listener of this.listeners) listener();
  };
}

export function sendUiAction(action: UiAction): void {
  const handler = window.webkit?.messageHandlers?.floatLyrics;
  if (!handler) return;
  handler.postMessage(JSON.stringify(action));
}

export const uiStore = new UiStore();
