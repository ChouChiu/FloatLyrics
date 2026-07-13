// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { LyricSlot, LyricsViewport } from "./app";
import { installLyricsBridge } from "./bridge";
import { findSyllableRanges, syllableProgress } from "./karaoke";
import { advanceLyricsViewState, initialLyricsViewState } from "./store";
import type { LyricsPayload, TimedSyllable } from "./types";

const style = {
  font_family: "Sans",
  lyric_font_px: 24,
  romanization_font_px: 12,
  translation_font_px: 13,
  played_color: "white",
  unplayed_color: "gray",
  romanization_color: "lightblue",
  translation_color: "white",
  transition_ms: 180,
};

function payload(key: string, text: string): LyricsPayload {
  return {
    key,
    content: { text, karaoke: null, romanization: "romaji", translation: "translation" },
    style,
  };
}

describe("lyrics view state", () => {
  test("uses the first slot without scheduling a transition", () => {
    const state = advanceLyricsViewState(initialLyricsViewState, payload("line:1", "first"));
    expect(state.activeSlot).toBe(0);
    expect(state.transitionRevision).toBe(0);
    expect(state.slots[0]?.content.text).toBe("first");
  });

  test("updates the active slot for the same key", () => {
    const initial = advanceLyricsViewState(initialLyricsViewState, payload("line:1", "first"));
    const updated = advanceLyricsViewState(initial, payload("line:1", "updated"));
    expect(updated.activeSlot).toBe(0);
    expect(updated.transitionRevision).toBe(0);
    expect(updated.slots[0]?.content.text).toBe("updated");
  });

  test("switches slots and preserves the outgoing line for a new key", () => {
    const initial = advanceLyricsViewState(initialLyricsViewState, payload("line:1", "first"));
    const updated = advanceLyricsViewState(initial, payload("line:2", "second"));
    expect(updated.activeSlot).toBe(1);
    expect(updated.transitionRevision).toBe(1);
    expect(updated.slots.map((slot) => slot?.content.text)).toEqual(["first", "second"]);
  });
});

describe("WebKit bridge", () => {
  test("delivers a payload queued before React initializes", () => {
    const pendingPayload = payload("line:1", "queued");
    const host = { floatLyricsPendingPayload: pendingPayload };
    const delivered: LyricsPayload[] = [];

    const bridge = installLyricsBridge(host, (value) => delivered.push(value));

    expect(delivered).toEqual([pendingPayload]);
    expect(host.floatLyricsPendingPayload).toBeUndefined();
    bridge.render(payload("line:2", "live"));
    expect(delivered[1]?.key).toBe("line:2");
  });
});

describe("karaoke progress", () => {
  const syllable: TimedSyllable = { text: "word", start_ms: 100, end_ms: 200 };

  test("finds repeated syllables from left to right", () => {
    expect(
      findSyllableRanges("Please Please", [
        { text: "Please", start_ms: 0, end_ms: 1 },
        { text: " ", start_ms: 1, end_ms: 2 },
        { text: "Please", start_ms: 2, end_ms: 3 },
      ]),
    ).toEqual([
      { start: 0, end: 6 },
      { start: 6, end: 7 },
      { start: 7, end: 13 },
    ]);
  });

  test("clamps progress at timing boundaries", () => {
    expect(syllableProgress(syllable, 99)).toBe(0);
    expect(syllableProgress(syllable, 150)).toBe(0.5);
    expect(syllableProgress(syllable, 200)).toBe(1);
  });

  test("completes zero-duration syllables", () => {
    expect(syllableProgress({ ...syllable, end_ms: 100 }, 100)).toBe(1);
  });
});

describe("React markup", () => {
  test("renders plain lyrics and secondary text", () => {
    const state = advanceLyricsViewState(initialLyricsViewState, payload("line:1", "歌词"));
    const html = renderToStaticMarkup(<LyricsViewport state={state} />);
    expect(html).toContain('id="viewport"');
    expect(html.match(/class="slot"/g)).toHaveLength(2);
    expect(html).toContain('class="primary plain"');
    expect(html).toContain("romaji");
    expect(html).toContain("translation");
  });

  test("renders karaoke text in base and played layers", () => {
    const content = payload("line:1", "ignored").content;
    content.karaoke = {
      text: "karaoke",
      position_ms: 150,
      syllables: [{ text: "karaoke", start_ms: 100, end_ms: 200 }],
    };
    const html = renderToStaticMarkup(<LyricSlot snapshot={{ key: "line:1", content }} />);
    expect(html).toContain('class="primary"');
    expect(html.match(/karaoke/g)).toHaveLength(2);
  });
});
