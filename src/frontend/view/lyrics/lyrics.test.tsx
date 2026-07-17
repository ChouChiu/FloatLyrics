// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import { describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { currentAmllLines, documentToAmllLines, resolvedLineEnd } from "./amll";
import { installLyricsBridge } from "./bridge";
import { findSyllableRanges, syllableProgress } from "./karaoke";
import { advanceLyricsViewState, initialLyricsViewState } from "./store";
import type {
  LyricsCommand,
  LyricsDocument,
  LyricsFrame,
  PresentedLyricLine,
  TimedSyllable,
} from "./types";

if (!("MouseEvent" in globalThis)) {
  Object.assign(globalThis, { MouseEvent: class MouseEvent extends Event {} });
}
const { AppleMusicLyrics, LyricSlot, LyricsViewport } = await import("./app");

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

function frame(key: string, text: string): LyricsFrame {
  return {
    key,
    content: { text, karaoke: null, romanization: "romaji", translation: "translation" },
    position_ms: 100,
    playing: true,
    seeking: false,
  };
}

function frameCommand(key: string, text: string): LyricsCommand {
  return { type: "frame", frame: frame(key, text) };
}

function configuredState() {
  return advanceLyricsViewState(initialLyricsViewState, {
    type: "configure",
    apple_music_style: false,
    style,
  });
}

describe("lyrics view state", () => {
  test("uses the first slot without scheduling a transition", () => {
    const state = advanceLyricsViewState(configuredState(), frameCommand("line:1", "first"));
    expect(state.activeSlot).toBe(0);
    expect(state.transitionRevision).toBe(0);
    expect(state.slots[0]?.content.text).toBe("first");
  });

  test("updates the active slot for the same key", () => {
    const initial = advanceLyricsViewState(configuredState(), frameCommand("line:1", "first"));
    const updated = advanceLyricsViewState(initial, frameCommand("line:1", "updated"));
    expect(updated.activeSlot).toBe(0);
    expect(updated.transitionRevision).toBe(0);
    expect(updated.slots[0]?.content.text).toBe("updated");
  });

  test("switches slots and preserves the outgoing line for a new key", () => {
    const initial = advanceLyricsViewState(configuredState(), frameCommand("line:1", "first"));
    const updated = advanceLyricsViewState(initial, frameCommand("line:2", "second"));
    expect(updated.activeSlot).toBe(1);
    expect(updated.transitionRevision).toBe(1);
    expect(updated.slots.map((slot) => slot?.content.text)).toEqual(["first", "second"]);
  });
});

describe("WebKit bridge", () => {
  test("delivers queued commands before React initializes", () => {
    const pendingCommand = frameCommand("line:1", "queued");
    const host = { floatLyricsPendingCommands: [pendingCommand] };
    const delivered: LyricsCommand[] = [];

    const bridge = installLyricsBridge(host, (value) => delivered.push(value));

    expect(delivered).toEqual([pendingCommand]);
    expect(host.floatLyricsPendingCommands).toBeUndefined();
    bridge.dispatch(frameCommand("line:2", "live"));
    expect(delivered[1]).toEqual(frameCommand("line:2", "live"));
  });
});

describe("AMLL conversion", () => {
  const line: PresentedLyricLine = {
    start_ms: 1_000,
    end_ms: null,
    text: "Hello",
    syllables: [],
    romanization: "hello",
    translation: "你好",
    background: "echo",
  };

  test("resolves missing line ends in document order", () => {
    expect(resolvedLineEnd(line, { ...line, start_ms: 2_000 }, 4_000)).toBe(2_000);
    expect(
      resolvedLineEnd(
        { ...line, syllables: [{ text: "Hi", start_ms: 1_000, end_ms: 1_800 }] },
        undefined,
        4_000,
      ),
    ).toBe(1_800);
    expect(resolvedLineEnd(line, undefined, 4_000)).toBe(4_000);
    expect(resolvedLineEnd(line, undefined, null)).toBe(6_000);
  });

  test("maps primary, secondary, timed, and background lyrics", () => {
    const document: LyricsDocument = {
      revision: 1,
      duration_ms: 4_000,
      lines: [{ ...line, syllables: [{ text: "Hello", start_ms: 1_000, end_ms: 1_800 }] }],
    };
    const lines = documentToAmllLines(document);
    expect(lines).toHaveLength(2);
    expect(lines[0]).toMatchObject({
      translatedLyric: "你好",
      romanLyric: "hello",
      isBG: false,
      words: [{ word: "Hello", startTime: 1_000, endTime: 1_800 }],
    });
    expect(lines[1]).toMatchObject({ isBG: true, words: [{ word: "echo" }] });
  });

  test("selects only the current primary line and its background vocal", () => {
    const document: LyricsDocument = {
      revision: 1,
      duration_ms: 4_000,
      lines: [line, { ...line, start_ms: 2_000, text: "World", background: "reply" }],
    };
    const lines = documentToAmllLines(document);
    const current = currentAmllLines(lines, "line:1");
    expect(current).toHaveLength(2);
    expect(current[0]?.words[0]?.word).toBe("World");
    expect(current[1]).toMatchObject({ isBG: true, words: [{ word: "reply" }] });
    expect(currentAmllLines(lines, "before-first-line")).toEqual([]);
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
    const state = advanceLyricsViewState(configuredState(), frameCommand("line:1", "歌词"));
    const html = renderToStaticMarkup(<LyricsViewport state={state} />);
    expect(html).toContain('id="viewport"');
    expect(html.match(/class="slot"/g)).toHaveLength(2);
    expect(html).toContain('class="primary plain"');
    expect(html).toContain("romaji");
    expect(html).toContain("translation");
  });

  test("renders karaoke text in base and played layers", () => {
    const content = frame("line:1", "ignored").content;
    content.karaoke = {
      text: "karaoke",
      position_ms: 150,
      syllables: [{ text: "karaoke", start_ms: 100, end_ms: 200 }],
    };
    const html = renderToStaticMarkup(<LyricSlot snapshot={{ key: "line:1", content }} />);
    expect(html).toContain('class="primary"');
    expect(html.match(/karaoke/g)).toHaveLength(2);
  });

  test("uses two FloatLyrics transition slots for AMLL lines", () => {
    const document: LyricsDocument = {
      revision: 1,
      duration_ms: 4_000,
      lines: [
        {
          start_ms: 0,
          end_ms: 2_000,
          text: "first",
          syllables: [],
          romanization: "",
          translation: "",
          background: "",
        },
        {
          start_ms: 2_000,
          end_ms: 4_000,
          text: "second",
          syllables: [],
          romanization: "",
          translation: "",
          background: "",
        },
      ],
    };
    let state = advanceLyricsViewState(initialLyricsViewState, {
      type: "configure",
      apple_music_style: true,
      style,
    });
    state = advanceLyricsViewState(state, { type: "document", document });
    state = advanceLyricsViewState(state, frameCommand("line:0", "first"));
    state = advanceLyricsViewState(state, frameCommand("line:1", "second"));

    const html = renderToStaticMarkup(<AppleMusicLyrics state={state} />);
    expect(html.match(/class="slot apple-music-slot"/g)).toHaveLength(2);
    expect(
      html.match(/class="apple-music-player" style="text-align:center;white-space:nowrap"/g),
    ).toHaveLength(2);
    expect(html).toContain('data-lyric-key="line:0"');
    expect(html).toContain('data-lyric-key="line:1"');
  });

  test("passes configured AMLL styles without overriding its unplayed color", () => {
    const document: LyricsDocument = {
      revision: 1,
      duration_ms: 2_000,
      lines: [
        {
          start_ms: 0,
          end_ms: 2_000,
          text: "first",
          syllables: [],
          romanization: "romanization",
          translation: "translation",
          background: "",
        },
      ],
    };
    let state = advanceLyricsViewState(initialLyricsViewState, {
      type: "configure",
      apple_music_style: true,
      style: {
        ...style,
        font_family: "Configured Font",
        lyric_font_px: 31,
        romanization_font_px: 11,
        translation_font_px: 17,
        played_color: "red",
        unplayed_color: "gray",
        romanization_color: "purple",
        translation_color: "orange",
      },
    });
    state = advanceLyricsViewState(state, { type: "document", document });
    state = advanceLyricsViewState(state, frameCommand("line:0", "first"));

    const html = renderToStaticMarkup(<AppleMusicLyrics state={state} />);
    expect(html).toContain("--amll-lp-font-family:Configured Font");
    expect(html).toContain("--amll-lp-font-size:31px");
    expect(html).toContain("--amll-lp-romanization-font-size:11px");
    expect(html).toContain("--amll-lp-translation-font-size:17px");
    expect(html).toContain("--amll-lp-color:red");
    expect(html).toContain("--amll-lp-romanization-color:purple");
    expect(html).toContain("--amll-lp-translation-color:orange");
    expect(html).not.toContain("--unplayed-color");
  });

  test("shows the waiting ellipsis before the first AMLL line", () => {
    let state = advanceLyricsViewState(initialLyricsViewState, {
      type: "configure",
      apple_music_style: true,
      style,
    });
    state = advanceLyricsViewState(state, frameCommand("before-first-line", "…"));

    const html = renderToStaticMarkup(<AppleMusicLyrics state={state} />);
    expect(html).toContain('class="primary plain"');
    expect(html).toContain("…");
    expect(html).not.toContain("apple-music-slot");
  });
});
