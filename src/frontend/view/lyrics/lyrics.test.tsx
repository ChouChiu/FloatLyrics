// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import { describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { clampAmllPosition, currentAmllLines, documentToAmllLines, resolvedLineEnd } from "./amll";
import { installLyricsBridge } from "./bridge";
import { Select, Slider } from "./components/ui";
import { type FadeTarget, type FrameScheduler, SlotFader } from "./fade";
import { findSyllableRanges, karaokeFillStyle, syllableProgress } from "./karaoke";
import { appendUnique, moveItem, removeItem } from "./list";
import { joinColor, splitColor } from "./settings/fields";
import { advanceLyricsViewState, initialLyricsViewState } from "./store";
import type {
  AppConfig,
  LyricsCommand,
  LyricsDocument,
  LyricsFrame,
  PresentedLyricLine,
  TimedSyllable,
} from "./types";
import { advanceUiState, type UiState } from "./ui-store";

if (!("MouseEvent" in globalThis)) {
  Object.assign(globalThis, { MouseEvent: class MouseEvent extends Event {} });
}

const { AppleMusicLyrics, LyricSlot, LyricsViewport } = await import("./app");
const { ControlCenter } = await import("./settings/control-center");
const { FontPickerWindow } = await import("./font-picker");
const { OverlayShell } = await import("./overlay");

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
  word_fade_width: 0,
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

class ManualFrames implements FrameScheduler {
  time = 0;
  private callbacks = new Map<number, (now: number) => void>();
  private nextHandle = 1;

  request(callback: (now: number) => void): number {
    const handle = this.nextHandle++;
    this.callbacks.set(handle, callback);
    return handle;
  }

  cancel(handle: number): void {
    this.callbacks.delete(handle);
  }

  now(): number {
    return this.time;
  }

  advance(ms: number): void {
    this.time += ms;
    const pending = [...this.callbacks.values()];
    this.callbacks.clear();
    for (const callback of pending) callback(this.time);
  }

  get scheduled(): number {
    return this.callbacks.size;
  }
}

function fadeTarget(opacity = ""): FadeTarget {
  return { style: { opacity } };
}

describe("slot cross-fade", () => {
  // WebKitGTK stopped presenting frames while a compositor opacity animation
  // ran, so the fade has to write opacity itself on every frame.
  test("writes the opacity of both slots on every frame of the fade", () => {
    const frames = new ManualFrames();
    const fader = new SlotFader(frames);
    const incoming = fadeTarget("0");
    const outgoing = fadeTarget("1");

    fader.crossFade(incoming, outgoing, 180);
    expect(incoming.style.opacity).toBe("0");
    expect(outgoing.style.opacity).toBe("1");

    frames.advance(90);
    expect(Number(incoming.style.opacity)).toBeCloseTo(0.5);
    expect(Number(outgoing.style.opacity)).toBeCloseTo(0.5);

    frames.advance(90);
    expect(incoming.style.opacity).toBe("1");
    expect(outgoing.style.opacity).toBe("0");
    expect(frames.scheduled).toBe(0);
  });

  test("continues an interrupted fade from the current opacity", () => {
    const frames = new ManualFrames();
    const fader = new SlotFader(frames);
    const first = fadeTarget("0");
    const second = fadeTarget("1");
    fader.crossFade(first, second, 180);
    frames.advance(90);

    fader.crossFade(second, first, 180);

    expect(Number(second.style.opacity)).toBeCloseTo(0.5);
    expect(Number(first.style.opacity)).toBeCloseTo(0.5);
    expect(frames.scheduled).toBe(1);
    frames.advance(180);
    expect(second.style.opacity).toBe("1");
    expect(first.style.opacity).toBe("0");
  });

  test("switches immediately without a transition duration", () => {
    const frames = new ManualFrames();
    const fader = new SlotFader(frames);
    const incoming = fadeTarget();
    const outgoing = fadeTarget();

    fader.crossFade(incoming, outgoing, 0);

    expect(incoming.style.opacity).toBe("1");
    expect(outgoing.style.opacity).toBe("0");
    expect(frames.scheduled).toBe(0);
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

describe("React application shell", () => {
  const config: AppConfig = {
    general: { language: "en", mode: "floating" },
    window: {
      anchor: "bottom-center",
      remember_position: true,
      position: null,
      margin: 96,
      width: 350,
      opacity: 0.78,
      bottom_panel_height: 36,
    },
    lyrics: {
      apple_music_style: false,
      offset_ms: 0,
      provider_order: ["qq-music", "netease"],
      show_translation: true,
      show_romanization: false,
      chinese_romanization: "auto",
      font_order: ["Sans"],
      lyric_font_size: 24,
      translation_font_size: 13,
      romanization_font_size: 12,
      played_color: "#FFFFFFFF",
      unplayed_color: "#9EA6B3FF",
      translation_color: "#FFFFFFC7",
      romanization_color: "#B8D8F0E6",
      transition_ms: 180,
      word_fade_width: 0,
      track_offset_step_ms: 100,
    },
    player: { preferred_players: ["spotify"], ignored_players: [] },
    amll: { address: "localhost:11444" },
    tray: { enabled: true },
  };
  const limits = {
    "window.width": { min: 320, max: 640 },
    "window.margin": { min: 0, max: 500 },
    "window.bottom_panel_height": { min: 0, max: 200 },
    "window.opacity": { min: 0.15, max: 1 },
    "lyrics.offset_ms": { min: -10000, max: 10000 },
    "lyrics.lyric_font_size": { min: 12, max: 56 },
    "lyrics.translation_font_size": { min: 8, max: 36 },
    "lyrics.romanization_font_size": { min: 8, max: 36 },
    "lyrics.transition_ms": { min: 0, max: 1000 },
    "lyrics.word_fade_width": { min: 0, max: 2 },
    "lyrics.track_offset_step_ms": { min: 10, max: 1000 },
  };
  const initial: UiState = {
    surface: null,
    config: null,
    defaults: null,
    limits: {},
    strings: {},
    version: "",
    about: { dependencies: [], licenses: [] },
    availableFonts: [],
    page: "general",
    saved: false,
    saving: false,
    saveError: null,
    search: null,
    songInfo: "FloatLyrics",
    trackOffset: "0 ms",
    snapClasses: [],
    panelOpacity: 0.78,
  };

  function bootstrapped(
    surface: UiState["surface"] & string,
    options: {
      config?: AppConfig;
      strings?: Record<string, string>;
      fonts?: string[];
    } = {},
  ): UiState {
    return advanceUiState(initial, {
      type: "bootstrap",
      surface: surface as "overlay",
      config: options.config ?? config,
      defaults: config,
      limits,
      strings: options.strings ?? {},
      version: "1.2.0",
      about: { dependencies: [], licenses: [] },
      available_fonts: options.fonts ?? [],
    });
  }

  function page(state: UiState, target: UiState["page"]): string {
    return renderToStaticMarkup(
      <ControlCenter state={advanceUiState(state, { type: "navigate", page: target })} />,
    );
  }

  test("bootstraps the selected surface with config, defaults, limits, and translations", () => {
    const state = bootstrapped("control-center", { strings: { General: "General" } });
    expect(state.surface).toBe("control-center");
    expect(state.config).toEqual(config);
    expect(state.defaults).toEqual(config);
    expect(state.limits["lyrics.transition_ms"]).toEqual({ min: 0, max: 1000 });
    expect((state.strings as { General?: string }).General).toBe("General");
  });

  test("draws dropdowns in the page instead of the native select popup", () => {
    const html = renderToStaticMarkup(
      <Select
        label="Language"
        value="zh-CN"
        options={[
          { value: "en", label: "English" },
          { value: "zh-CN", label: "简体中文" },
        ]}
        onValueChange={() => {}}
      />,
    );
    expect(html).not.toContain("<select");
    expect(html).toContain('role="combobox"');
    expect(html).toContain('aria-expanded="false"');
    expect(html).toContain(">简体中文<");
    expect(page(bootstrapped("control-center"), "general")).not.toContain("<select");
  });

  test("keeps control-center navigation independent from lyrics frames", () => {
    const about = advanceUiState(initial, { type: "navigate", page: "about" });
    expect(about.page).toBe("about");
    expect(advanceUiState(about, frameCommand("line:1", "lyrics"))).toBe(about);
  });

  test("lays display settings out as sections with the backend's ranges", () => {
    const html = page(
      bootstrapped("control-center", {
        strings: { Animation: "Animation", LineTransition: "Line transition" },
      }),
      "display",
    );
    expect(html).not.toContain("section-tabs");
    expect(html).toContain(">Animation<");
    expect(html).toContain("Line transition");
    expect(html).toContain('max="1000"');
    expect(html).toContain('max="2"');
  });

  test("offers a reset only for values that differ from their default", () => {
    const changed = structuredClone(config);
    changed.lyrics.transition_ms = 400;
    const html = page(bootstrapped("control-center", { config: changed }), "display");
    expect(html.match(/data-visible="true"/g)).toHaveLength(1);
    expect(page(bootstrapped("control-center"), "display")).not.toContain('data-visible="true"');
  });

  test("enables the AMLL address only in AMLL mode", () => {
    const floating = page(bootstrapped("control-center"), "integration");
    expect(floating).toContain('value="localhost:11444"');
    expect(floating).toContain("disabled");
    expect(floating).toContain('role="switch"');

    const amll = page(
      bootstrapped("control-center", {
        config: { ...config, general: { language: "en", mode: "amll" } },
      }),
      "integration",
    );
    expect(amll).not.toContain("disabled");
  });

  test("edits ordered preference lists without losing their last entry", () => {
    const sources = ["qq-music", "kugou", "lrclib"];
    expect(appendUnique(sources, "netease")).toEqual([...sources, "netease"]);
    expect(appendUnique(sources, "kugou")).toBe(sources);
    expect(appendUnique(["Sans"], " ")).toEqual(["Sans"]);
    expect(moveItem(sources, 2, -1)).toEqual(["qq-music", "lrclib", "kugou"]);
    expect(moveItem(sources, 0, -1)).toBe(sources);
    expect(removeItem(sources, 1)).toEqual(["qq-music", "lrclib"]);
    const single = ["Sans"];
    expect(removeItem(single, 0)).toBe(single);
  });

  test("lists enabled sources in order and offers the rest", () => {
    const html = page(
      bootstrapped("control-center", {
        strings: {
          AvailableSources: "Add a source",
          MoveSourceDown: "Move source down",
          RemoveSource: "Remove source",
          ProviderNameQqMusic: "QQ Music",
          ProviderNameNetEase: "NetEase Cloud Music",
          ProviderNameKugou: "Kugou Music",
        },
      }),
      "sources",
    );
    expect(html.indexOf("QQ Music")).toBeLessThan(html.indexOf("NetEase Cloud Music"));
    expect(html).toContain('title="Move source down"');
    expect(html).toContain('title="Remove source"');
    expect(html).toContain("Add a source");
    expect(html.match(/QQ Music/g)?.length).toBe(1);
    expect(html.match(/Kugou Music/g)?.length).toBe(1);
  });

  test("renders the two-column font picker with a filter", () => {
    const html = renderToStaticMarkup(
      <FontPickerWindow
        state={bootstrapped("font-picker", {
          strings: { AvailableFonts: "Available fonts", FontOrder: "Font priority" },
          fonts: ["Noto Sans", "Source Han Sans"],
        })}
      />,
    );
    expect(html).toContain("Available fonts");
    expect(html).toContain("Font priority");
    expect(html).toContain('type="search"');
    expect(html).toContain("Noto Sans");
    expect(html).toContain('class="font-row selected-font-row"');
  });

  test("keeps the alpha channel when a color changes", () => {
    expect(splitColor("#B8D8F0E6")).toEqual({ rgb: "#b8d8f0", alpha: 90 });
    expect(splitColor("#123456")).toEqual({ rgb: "#123456", alpha: 100 });
    expect(joinColor("#ff0000", 90)).toBe("#FF0000E6");
    expect(joinColor("#ff0000", 140)).toBe("#FF0000FF");
  });

  test("applies native snap edges to the overlay shell", () => {
    const snapped = advanceUiState(initial, {
      type: "overlay-placement",
      classes: ["snapped-left", "snapped-bottom"],
    });

    expect(snapped.snapClasses).toEqual(["snapped-left", "snapped-bottom"]);
    expect(renderToStaticMarkup(<OverlayShell state={snapped} />)).toContain(
      'class="overlay-shell snapped-left snapped-bottom"',
    );
  });

  test("applies overlay opacity updates without another bootstrap", () => {
    const updated = advanceUiState(initial, { type: "overlay-appearance", opacity: 0.35 });

    expect(updated.panelOpacity).toBe(0.35);
    expect(renderToStaticMarkup(<OverlayShell state={updated} />)).toContain(
      'style="--panel-opacity:0.35"',
    );
  });

  test("names the configured timing step on the overlay buttons", () => {
    const stepped = structuredClone(config);
    stepped.lyrics.track_offset_step_ms = 250;
    const html = renderToStaticMarkup(
      <OverlayShell
        state={bootstrapped("overlay", {
          config: stepped,
          strings: { IncreaseTrackOffsetTooltip: "Show lyrics {step} ms earlier" },
        })}
      />,
    );
    expect(html).toContain('title="Show lyrics 250 ms earlier"');
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
    background_translation: "回声",
    background_start_ms: 1_000,
    background_end_ms: null,
    background_syllables: [],
    voice: "primary",
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

  test("keeps a QRC background vocal on its own word timings", () => {
    const document: LyricsDocument = {
      revision: 1,
      duration_ms: 208_000,
      lines: [
        {
          ...line,
          start_ms: 55_212,
          end_ms: 58_847,
          background: "echo",
          background_start_ms: 58_082,
          background_end_ms: 58_847,
          background_syllables: [
            { text: "e", start_ms: 58_082, end_ms: 58_200 },
            { text: "cho", start_ms: 58_200, end_ms: 58_847 },
          ],
        },
      ],
    };

    const [main, background] = documentToAmllLines(document);
    expect(main).toMatchObject({ startTime: 55_212, endTime: 58_847 });
    expect(background).toMatchObject({
      isBG: true,
      startTime: 58_082,
      endTime: 58_847,
      words: [
        { word: "e", startTime: 58_082, endTime: 58_200 },
        { word: "cho", startTime: 58_200, endTime: 58_847 },
      ],
    });
  });

  test("uses the background line's timing when it only has text", () => {
    const document: LyricsDocument = {
      revision: 1,
      duration_ms: 4_000,
      lines: [{ ...line, background_start_ms: 2_500, background_end_ms: 3_000 }],
    };

    expect(documentToAmllLines(document)[1]).toMatchObject({
      startTime: 2_500,
      endTime: 3_000,
      words: [{ word: "echo", startTime: 2_500, endTime: 3_000 }],
    });
  });

  test("keeps advancing while the background vocal outlasts the main line", () => {
    const document: LyricsDocument = {
      revision: 1,
      duration_ms: 4_000,
      lines: [
        {
          ...line,
          end_ms: 2_000,
          background_start_ms: 2_500,
          background_end_ms: 3_000,
        },
      ],
    };
    const current = currentAmllLines(documentToAmllLines(document), "line:0");

    expect(clampAmllPosition(current, 2_750)).toBe(2_750);
    expect(clampAmllPosition(current, 3_250)).toBe(2_999);
  });

  test("attaches syllable readings to their own word", () => {
    const document: LyricsDocument = {
      revision: 1,
      duration_ms: 4_000,
      lines: [
        {
          ...line,
          romanization: "annyeong segye",
          syllables: [
            { text: "안녕", start_ms: 1_000, end_ms: 1_500, romanization: "annyeong" },
            { text: " ", start_ms: 1_500, end_ms: 1_600 },
            { text: "세계", start_ms: 1_600, end_ms: 2_000, romanization: "segye" },
          ],
        },
      ],
    };
    const lines = documentToAmllLines(document);

    expect(lines[0]?.words.map((word) => word.romanWord)).toEqual(["annyeong", "", "segye"]);
    // Per-word readings replace the line-level romanization in AMLL.
    expect(lines[0]?.romanLyric).toBe("");
  });

  test("keeps the line-level romanization when no word has a reading", () => {
    const document: LyricsDocument = {
      revision: 1,
      duration_ms: 4_000,
      lines: [
        {
          ...line,
          romanization: "konnichiha",
          syllables: [{ text: "こんにちは", start_ms: 1_000, end_ms: 2_000 }],
        },
      ],
    };
    const lines = documentToAmllLines(document);

    expect(lines[0]?.words[0]?.romanWord).toBe("");
    expect(lines[0]?.romanLyric).toBe("konnichiha");
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

  test("reveals the fill with a hard edge unless a fade width is set", () => {
    expect(karaokeFillStyle(30, 100, 0)).toEqual({
      clipPath: "inset(0 70px 0 0)",
      maskImage: "none",
    });
    expect(karaokeFillStyle(30, 100, 10)).toEqual({
      clipPath: "none",
      maskImage: "linear-gradient(to right, #000 25.0px, transparent 35.0px)",
    });
  });

  test("collapses the soft edge at both ends of the line", () => {
    expect(karaokeFillStyle(-5, 100, 10).clipPath).toBe("inset(0 100px 0 0)");
    expect(karaokeFillStyle(140, 100, 10).clipPath).toBe("inset(0 0px 0 0)");
  });

  test("completes zero-duration syllables", () => {
    expect(syllableProgress({ ...syllable, end_ms: 100 }, 100)).toBe(1);
  });
});

describe("React markup", () => {
  test("renders numeric inputs next to range sliders", () => {
    const html = renderToStaticMarkup(
      <Slider
        label="Background opacity"
        value={0.42}
        min={0.15}
        max={1}
        step={0.01}
        onValueChange={() => {}}
      />,
    );

    expect(html).toContain('type="range"');
    expect(html).toContain('type="number"');
    expect(html).toContain('value="0.42"');
    expect(html.match(/aria-label="Background opacity"/g)).toHaveLength(2);
  });

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

  test("uses slot-based AMLL players with custom line transitions", () => {
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
          background_translation: "",
          background_start_ms: 0,
          background_end_ms: null,
          background_syllables: [],
          voice: "primary",
        },
        {
          start_ms: 2_000,
          end_ms: 4_000,
          text: "second",
          syllables: [],
          romanization: "",
          translation: "",
          background: "",
          background_translation: "",
          background_start_ms: 0,
          background_end_ms: null,
          background_syllables: [],
          voice: "primary",
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
    expect(
      html.match(/class="apple-music-player" style="text-align:center;white-space:nowrap"/g),
    ).toHaveLength(2);
    expect(html.match(/class="slot apple-music-slot"/g)).toHaveLength(2);
    expect(html.match(/data-lyric-key="line:[01]"/g)).toHaveLength(2);
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
          background_translation: "",
          background_start_ms: 0,
          background_end_ms: null,
          background_syllables: [],
          voice: "primary",
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
