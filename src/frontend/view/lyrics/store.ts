// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

import type { LyricContent, LyricsPayload, LyricsStyle } from "./types";

export interface SlotSnapshot {
  key: string;
  content: LyricContent;
}

export interface LyricsViewState {
  currentKey: string | null;
  activeSlot: 0 | 1;
  slots: readonly [SlotSnapshot | null, SlotSnapshot | null];
  style: LyricsStyle | null;
  transitionRevision: number;
}

export const initialLyricsViewState: LyricsViewState = Object.freeze({
  currentKey: null,
  activeSlot: 0,
  slots: Object.freeze([null, null] as const),
  style: null,
  transitionRevision: 0,
});

export function advanceLyricsViewState(
  state: LyricsViewState,
  payload: LyricsPayload,
): LyricsViewState {
  const keyChanged = state.currentKey !== payload.key;
  const isFirstValue = state.currentKey === null;
  const activeSlot =
    keyChanged && !isFirstValue ? ((1 - state.activeSlot) as 0 | 1) : state.activeSlot;
  const slots: [SlotSnapshot | null, SlotSnapshot | null] = [...state.slots];
  slots[activeSlot] = { key: payload.key, content: payload.content };

  return {
    currentKey: payload.key,
    activeSlot,
    slots,
    style: payload.style,
    transitionRevision: state.transitionRevision + (keyChanged && !isFirstValue ? 1 : 0),
  };
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

  readonly render = (payload: LyricsPayload): void => {
    this.state = advanceLyricsViewState(this.state, payload);
    for (const listener of this.listeners) listener();
  };
}

export const lyricsStore = new LyricsStore();
