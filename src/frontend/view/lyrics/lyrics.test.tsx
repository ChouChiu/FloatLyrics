// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import { describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { currentAmllLines, documentToAmllLines, resolvedLineEnd } from "./amll";
import { installLyricsBridge } from "./bridge";
import { AppIcon, Icon, Slider } from "./components/ui";
import { findSyllableRanges, syllableProgress } from "./karaoke";
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
const {
  addFontFamily,
  addLyricsProvider,
  ControlCenter,
  FontPickerWindow,
  ManualSearchWindow,
  moveFontFamily,
  moveLyricsProvider,
  OverlayShell,
  removeFontFamily,
  removeLyricsProvider,
} = await import("./shell");

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
    },
    player: { preferred_players: ["spotify"], ignored_players: [] },
    amll: { address: "localhost:11444" },
    tray: { enabled: true },
  };
  const initial: UiState = {
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

  test("bootstraps the selected React surface with config and translations", () => {
    const state = advanceUiState(initial, {
      type: "bootstrap",
      surface: "control-center",
      config,
      strings: { General: "General" },
      version: "1.1.2",
      about: { dependencies: [], licenses: [] },
      available_fonts: [],
    });
    expect(state.surface).toBe("control-center");
    expect(state.config).toEqual(config);
    const strings = state.strings as { General?: string };
    expect(strings.General).toBe("General");
  });

  test("keeps control-center navigation independent from lyrics frames", () => {
    const about = advanceUiState(initial, { type: "navigate", page: "about" });
    expect(about.page).toBe("about");
    expect(advanceUiState(about, frameCommand("line:1", "lyrics"))).toBe(about);
  });

  test("uses the sidebar for settings pages and top tabs for display subpages", () => {
    let state = advanceUiState(initial, {
      type: "bootstrap",
      surface: "control-center",
      config,
      strings: {
        General: "General",
        Display: "Display",
        LyricsSources: "Lyrics Sources",
        About: "About",
        DisplayTitle: "Display",
        DisplayDescription: "Display settings",
        Panel: "Panel",
        Fonts: "Fonts",
        Colors: "Colors",
      },
      version: "1.1.2",
      about: { dependencies: [], licenses: [] },
      available_fonts: [],
    });
    state = advanceUiState(state, { type: "navigate", page: "display" });

    const html = renderToStaticMarkup(<ControlCenter state={state} />);
    expect(html).toContain("Lyrics Sources");
    expect(html).toContain('class="section-tabs"');
    expect(html).toContain(">Panel<");
    expect(html).toContain(">Fonts<");
    expect(html).toContain(">Colors<");
  });

  test("renders the integration page with mode, address, and tray controls", () => {
    const strings = {
      Integration: "Integration",
      RunMode: "Run mode",
      RunModeDescription: "The floating overlay and the AMLL sender cannot run at the same time",
      RunModeFloating: "Floating overlay",
      RunModeAmll: "AMLL WebSocket sender",
      RunModeRestartHint: "Restart FloatLyrics to apply this change",
      AmllAddress: "AMLL player address",
      AmllAddressDescription: "Host and port",
      TrayIcon: "System tray icon",
      TrayIconDescription: "Show the icon",
      ChangesSavedAutomatically: "Changes are saved automatically",
    };
    const bootstrapped = (incoming: AppConfig) =>
      advanceUiState(initial, {
        type: "bootstrap",
        surface: "control-center",
        config: incoming,
        strings,
        version: "1.2.0",
        about: { dependencies: [], licenses: [] },
        available_fonts: [],
      });
    const integration = advanceUiState(bootstrapped(config), {
      type: "navigate",
      page: "integration",
    });

    const html = renderToStaticMarkup(<ControlCenter state={integration} />);
    expect(html).toContain("Run mode");
    expect(html).toContain('value="localhost:11444"');
    expect(html).toContain("disabled");
    expect(html).toContain('role="switch"');
    expect(html).toContain("Restart FloatLyrics to apply this change");

    const amll = advanceUiState(
      bootstrapped({ ...config, general: { language: "en", mode: "amll" } }),
      { type: "navigate", page: "integration" },
    );
    const amllHtml = renderToStaticMarkup(<ControlCenter state={amll} />);
    expect(amllHtml).not.toContain("disabled");
  });

  test("offers a quit action from the settings sidebar", () => {
    const state = advanceUiState(initial, {
      type: "bootstrap",
      surface: "control-center",
      config,
      strings: { Quit: "Quit FloatLyrics" },
      version: "1.2.0",
      about: { dependencies: [], licenses: [] },
      available_fonts: [],
    });

    expect(renderToStaticMarkup(<ControlCenter state={state} />)).toContain("Quit FloatLyrics");
  });

  test("renders manual search as a standalone window without the settings sidebar", () => {
    let state = advanceUiState(initial, {
      type: "bootstrap",
      surface: "manual-search",
      config,
      strings: {
        ManualSearchTitle: "Select Lyrics Manually",
        SearchAfterPlayback: "Search after playback starts",
        SelectCandidatePreview: "Select a candidate",
        Title: "Title",
        SongTitle: "Song title",
        Artist: "Artist",
        Search: "Search",
        ApplySelectedLyrics: "Apply",
      },
      version: "1.1.2",
      about: { dependencies: [], licenses: [] },
      available_fonts: [],
    });
    state = advanceUiState(state, {
      type: "search-state",
      state: {
        title: "Song",
        artist: "Artist",
        status: "Ready",
        preview: "Preview",
        searching: false,
        applying: false,
        can_apply: false,
        selected_index: null,
        candidates: [],
      },
    });

    const html = renderToStaticMarkup(<ManualSearchWindow state={state} />);
    expect(html).toContain('class="manual-search-window"');
    expect(html).toContain('id="manual-search-title"');
    expect(html).not.toContain('class="sidebar"');
  });

  test("reorders and enables lyrics sources", () => {
    expect(addLyricsProvider(["qq-music"], "kugou")).toEqual(["qq-music", "kugou"]);
    expect(addLyricsProvider(["qq-music"], "qq-music")).toEqual(["qq-music"]);
    expect(moveLyricsProvider(["qq-music", "kugou", "lrclib"], 2, -1)).toEqual([
      "qq-music",
      "lrclib",
      "kugou",
    ]);
    expect(moveLyricsProvider(["qq-music", "kugou"], 0, -1)).toEqual(["qq-music", "kugou"]);
    expect(removeLyricsProvider(["qq-music", "kugou", "lrclib"], 1)).toEqual([
      "qq-music",
      "lrclib",
    ]);
    expect(removeLyricsProvider(["qq-music"], 0)).toEqual(["qq-music"]);
  });

  test("renders the source order with the sources that are not enabled yet", () => {
    let state = advanceUiState(initial, {
      type: "bootstrap",
      surface: "control-center",
      config,
      strings: {
        General: "General",
        Display: "Display",
        LyricsSources: "Lyrics Sources",
        About: "About",
        SourcesTitle: "Lyrics Sources",
        SourcesDescription: "Search online sources in order",
        SearchPriority: "Search priority",
        SearchPriorityDescription: "Search the sources in this order",
        AvailableSources: "Add a source",
        MoveSourceUp: "Move source up",
        MoveSourceDown: "Move source down",
        RemoveSource: "Remove source",
        ProviderNameQqMusic: "QQ Music",
        ProviderNameNetEase: "NetEase Cloud Music",
        ProviderNameKugou: "Kugou Music",
        ProviderNameLrclib: "LRCLIB",
        ProviderNameSodaMusic: "Soda Music",
      },
      version: "1.1.2",
      about: { dependencies: [], licenses: [] },
      available_fonts: [],
    });
    state = advanceUiState(state, { type: "navigate", page: "sources" });

    const html = renderToStaticMarkup(<ControlCenter state={state} />);
    expect(html).toContain("Search priority");
    // Both enabled sources are rows of the ordered list, in their stored order.
    expect(html.indexOf("QQ Music")).toBeLessThan(html.indexOf("NetEase Cloud Music"));
    expect(html).toContain('title="Move source down"');
    expect(html).toContain('title="Remove source"');
    // A source that is not enabled is offered instead of being listed.
    expect(html).toContain("Add a source");
    expect(html).toContain(">Kugou Music<");
    // Only the enabled sources are rows of the list; the rest are offered.
    expect(html.match(/QQ Music/g)?.length).toBe(1);
    expect(html.match(/Kugou Music/g)?.length).toBe(1);
  });

  test("renders the original two-column font selection workflow", () => {
    const state = advanceUiState(initial, {
      type: "bootstrap",
      surface: "font-picker",
      config,
      strings: {
        AvailableFonts: "Available fonts",
        FontOrder: "Font priority",
        MoveFontUp: "Move font up",
        MoveFontDown: "Move font down",
        RemoveFont: "Remove font",
        Done: "Done",
      },
      version: "1.1.2",
      about: { dependencies: [], licenses: [] },
      available_fonts: ["Noto Sans", "Source Han Sans"],
    });

    const html = renderToStaticMarkup(<FontPickerWindow state={state} />);
    expect(html).toContain("Available fonts");
    expect(html).toContain("Font priority");
    expect(html).toContain("Noto Sans");
    expect(html).toContain('class="font-row selected-font-row"');
  });

  test("applies native snap edges to the React overlay shell", () => {
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

  test("completes zero-duration syllables", () => {
    expect(syllableProgress({ ...syllable, end_ms: 100 }, 100)).toBe(1);
  });
});

describe("React markup", () => {
  test("keeps font fallback order valid while adding, moving, and removing families", () => {
    const original = ["Sans"];
    expect(addFontFamily(original, "Sans")).toBe(original);
    expect(addFontFamily(original, "Noto Sans")).toEqual(["Sans", "Noto Sans"]);
    expect(moveFontFamily(["Sans", "Noto Sans"], 1, -1)).toEqual(["Noto Sans", "Sans"]);
    expect(removeFontFamily(original, 0)).toBe(original);
    expect(removeFontFamily(["Sans", "Noto Sans"], 0)).toEqual(["Noto Sans"]);
  });

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

  test("renders every shell icon through Remix Icon components", () => {
    for (const name of [
      "minus",
      "plus",
      "search",
      "settings",
      "display",
      "sources",
      "up",
      "down",
      "remove",
      "x",
      "info",
    ] as const) {
      const html = renderToStaticMarkup(<Icon name={name} />);
      expect(html).toContain("<svg");
      expect(html).toContain('aria-hidden="true"');
      expect(html).toContain('class="remixicon icon"');
      expect(html).toContain('fill="currentColor"');
    }
  });

  test("renders the application icon mark", () => {
    const html = renderToStaticMarkup(<AppIcon className="app-mark" />);
    expect(html).toContain('class="app-mark"');
    expect(html).toContain('aria-label="FloatLyrics"');
    expect(html).toContain("url(#app-icon-backdrop)");
    expect(html).toContain("url(#app-icon-accent)");
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
