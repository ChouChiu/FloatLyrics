// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import type {
  LyricContent,
  LyricsCommand,
  LyricsDocument,
  LyricsFrame,
  LyricsStyle,
} from "./types";

export interface SlotSnapshot {
  key: string;
  content: LyricContent;
}

export interface LyricsViewState {
  currentKey: string | null;
  activeSlot: 0 | 1;
  slots: readonly [SlotSnapshot | null, SlotSnapshot | null];
  style: LyricsStyle | null;
  appleMusicStyle: boolean;
  document: LyricsDocument | null;
  frame: LyricsFrame | null;
  transitionRevision: number;
}

export const initialLyricsViewState: LyricsViewState = Object.freeze({
  currentKey: null,
  activeSlot: 0,
  slots: Object.freeze([null, null] as const),
  style: null,
  appleMusicStyle: false,
  document: null,
  frame: null,
  transitionRevision: 0,
});

function applyFrame(state: LyricsViewState, frame: LyricsFrame): LyricsViewState {
  const keyChanged = state.currentKey !== frame.key;
  const isFirstValue = state.currentKey === null;
  const activeSlot =
    keyChanged && !isFirstValue ? ((1 - state.activeSlot) as 0 | 1) : state.activeSlot;
  const slots: [SlotSnapshot | null, SlotSnapshot | null] = [...state.slots];
  slots[activeSlot] = { key: frame.key, content: frame.content };

  return {
    ...state,
    currentKey: frame.key,
    activeSlot,
    slots,
    frame,
    transitionRevision: state.transitionRevision + (keyChanged && !isFirstValue ? 1 : 0),
  };
}

export function advanceLyricsViewState(
  state: LyricsViewState,
  command: LyricsCommand,
): LyricsViewState {
  switch (command.type) {
    case "configure":
      return {
        ...state,
        appleMusicStyle: command.apple_music_style,
        style: command.style,
      };
    case "document":
      return { ...state, document: command.document };
    case "frame":
      return applyFrame(state, command.frame);
    default:
      return state;
  }
}

type Listener = () => void;

class LyricsStore {
  private state = initialLyricsViewState;
  private readonly listeners = new Set<Listener>();

  readonly getSnapshot = (): LyricsViewState => this.state;

  readonly subscribe = (listener: Listener): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  readonly dispatch = (command: LyricsCommand): void => {
    const next = advanceLyricsViewState(this.state, command);
    // Every command reaches this store, so a command that does not touch the
    // lyrics view must not notify the subscribers.
    if (next === this.state) return;
    this.state = next;
    for (const listener of this.listeners) listener();
  };
}

export const lyricsStore = new LyricsStore();
