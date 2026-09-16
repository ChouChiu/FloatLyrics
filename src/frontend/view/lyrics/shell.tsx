// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import { useEffect, useState, useSyncExternalStore } from "react";
import { LyricsApp } from "./app";
import {
  AppIcon,
  Button,
  Card,
  Icon,
  Input,
  Select,
  SettingRow,
  Slider,
  Switch,
} from "./components/ui";
import type { AppConfig, ControlPage, SearchState } from "./types";
import { sendUiAction, type UiState, uiStore } from "./ui-store";

function text(state: UiState, key: string): string {
  return state.strings[key] ?? key;
}

export function OverlayShell({ state }: { state: UiState }) {
  return (
    <div
      className={["overlay-shell", ...state.snapClasses].join(" ")}
      style={{ "--panel-opacity": state.panelOpacity } as React.CSSProperties}
    >
      <header className="overlay-toolbar">
        <div className="track-title" title={state.songInfo}>
          {state.songInfo}
        </div>
        <div className="offset-control">
          <Button
            variant="ghost"
            size="icon"
            title={text(state, "DecreaseTrackOffsetTooltip")}
            onClick={() => sendUiAction({ type: "adjust-track-offset", delta_ms: -100 })}
          >
            <Icon name="minus" />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            title={text(state, "ResetTrackOffsetTooltip")}
            onClick={() => sendUiAction({ type: "reset-track-offset" })}
          >
            {state.trackOffset}
          </Button>
          <Button
            variant="ghost"
            size="icon"
            title={text(state, "IncreaseTrackOffsetTooltip")}
            onClick={() => sendUiAction({ type: "adjust-track-offset", delta_ms: 100 })}
          >
            <Icon name="plus" />
          </Button>
        </div>
        <Button
          variant="ghost"
          size="icon"
          title={text(state, "ManualSearchTooltip")}
          onClick={() => sendUiAction({ type: "open-search" })}
        >
          <Icon name="search" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          title={text(state, "OpenSettingsTooltip")}
          onClick={() => sendUiAction({ type: "open-settings" })}
        >
          <Icon name="settings" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          title={text(state, "CloseTooltip")}
          onClick={() => sendUiAction({ type: "quit" })}
        >
          <Icon name="x" />
        </Button>
      </header>
      <div className="overlay-divider" />
      <div className="lyrics-region">
        <LyricsApp />
      </div>
    </div>
  );
}

function cloneConfig(config: AppConfig): AppConfig {
  return structuredClone(config);
}

/**
 * Draft of the configuration a page edits.
 *
 * The page sends the whole configuration on every change, so the draft follows
 * the one the backend reports back and is `null` until the first state arrives.
 */
function useDraftConfig(
  state: UiState,
): [AppConfig | null, (change: (next: AppConfig) => void) => void] {
  const incoming = state.config;
  const [draft, setDraft] = useState<AppConfig | null>(incoming);
  useEffect(() => setDraft(incoming), [incoming]);

  const update = (change: (next: AppConfig) => void) => {
    if (!draft) return;
    const next = cloneConfig(draft);
    change(next);
    setDraft(next);
    sendUiAction({ type: "save-config", config: next });
  };
  return [draft, update];
}

type SettingsPageName = Exclude<ControlPage, "about" | "integration">;
type SettingsSubpage = "language" | "lyrics" | "panel" | "fonts" | "colors" | "sources";

const settingsPages: Record<
  SettingsPageName,
  {
    title: string;
    description: string;
    defaultSubpage: SettingsSubpage;
    tabs: Array<[SettingsSubpage, string]>;
  }
> = {
  general: {
    title: "GeneralTitle",
    description: "GeneralDescription",
    defaultSubpage: "language",
    tabs: [
      ["language", "Language"],
      ["lyrics", "LyricsContent"],
    ],
  },
  display: {
    title: "DisplayTitle",
    description: "DisplayDescription",
    defaultSubpage: "panel",
    tabs: [
      ["panel", "Panel"],
      ["fonts", "Fonts"],
      ["colors", "Colors"],
    ],
  },
  sources: {
    title: "SourcesTitle",
    description: "SourcesDescription",
    defaultSubpage: "sources",
    tabs: [["sources", "LyricsSources"]],
  },
};

function SettingsPage({ state, page }: { state: UiState; page: SettingsPageName }) {
  const [draft, update] = useDraftConfig(state);
  const metadata = settingsPages[page];
  const [subpage, setSubpage] = useState<SettingsSubpage>(metadata.defaultSubpage);
  if (!draft) return null;
  const t = (key: string) => text(state, key);

  return (
    <div className="page-stack">
      <div className="page-heading">
        <div>
          <h1>{t(metadata.title)}</h1>
          <p>{t(metadata.description)}</p>
        </div>
        <div className={state.saveError ? "save-state error" : "save-state"}>
          {state.saveError
            ? `${t("SaveFailed")}: ${state.saveError}`
            : state.saved
              ? t("Saved")
              : t("ChangesSavedAutomatically")}
        </div>
      </div>
      {metadata.tabs.length > 1 && (
        <div className="section-tabs">
          {metadata.tabs.map(([value, label]) => (
            <Button
              key={value}
              variant={subpage === value ? "default" : "ghost"}
              onClick={() => setSubpage(value)}
            >
              {t(label)}
            </Button>
          ))}
        </div>
      )}

      {subpage === "language" && (
        <Card>
          <SettingRow title={t("Language")} description={t("LanguageDescription")}>
            <Select
              value={draft.general.language}
              onChange={(event) =>
                update((next) => {
                  next.general.language = event.currentTarget
                    .value as AppConfig["general"]["language"];
                })
              }
            >
              <option value="en">English</option>
              <option value="zh-CN">简体中文</option>
              <option value="zh-TW">繁體中文</option>
            </Select>
          </SettingRow>
        </Card>
      )}

      {subpage === "lyrics" && (
        <Card>
          <SettingRow title={t("GlobalOffset")} description={t("GlobalOffsetDescription")}>
            <Slider
              label={t("GlobalOffset")}
              value={draft.lyrics.offset_ms}
              min={-10000}
              max={10000}
              step={50}
              unit=" ms"
              onValueChange={(value) =>
                update((next) => {
                  next.lyrics.offset_ms = value;
                })
              }
            />
          </SettingRow>
          <SettingRow title={t("ShowTranslation")} description={t("ShowTranslationDescription")}>
            <Switch
              label={t("ShowTranslation")}
              checked={draft.lyrics.show_translation}
              onCheckedChange={(value) =>
                update((next) => {
                  next.lyrics.show_translation = value;
                })
              }
            />
          </SettingRow>
          <SettingRow title={t("ShowRomanization")} description={t("ShowRomanizationDescription")}>
            <Switch
              label={t("ShowRomanization")}
              checked={draft.lyrics.show_romanization}
              onCheckedChange={(value) =>
                update((next) => {
                  next.lyrics.show_romanization = value;
                })
              }
            />
          </SettingRow>
          <SettingRow
            title={t("ChineseRomanization")}
            description={t("ChineseRomanizationDescription")}
          >
            <Select
              disabled={!draft.lyrics.show_romanization}
              value={draft.lyrics.chinese_romanization}
              onChange={(event) =>
                update((next) => {
                  next.lyrics.chinese_romanization = event.currentTarget
                    .value as AppConfig["lyrics"]["chinese_romanization"];
                })
              }
            >
              <option value="auto">{t("RomanizationAutomatic")}</option>
              <option value="mandarin-pinyin">{t("MandarinPinyin")}</option>
              <option value="cantonese-jyutping">{t("CantoneseJyutping")}</option>
              <option value="cantonese-jyutping-no-tones">
                {t("CantoneseJyutpingWithoutTones")}
              </option>
            </Select>
          </SettingRow>
        </Card>
      )}

      {subpage === "panel" && (
        <Card>
          <SettingRow title={t("AppleMusicStyle")} description={t("AppleMusicStyleDescription")}>
            <Switch
              label={t("AppleMusicStyle")}
              checked={draft.lyrics.apple_music_style}
              onCheckedChange={(value) =>
                update((next) => {
                  next.lyrics.apple_music_style = value;
                })
              }
            />
          </SettingRow>
          <SettingRow title={t("PanelWidth")} description={t("PanelWidthDescription")}>
            <Slider
              label={t("PanelWidth")}
              value={draft.window.width}
              min={320}
              max={640}
              step={10}
              unit=" px"
              onValueChange={(value) =>
                update((next) => {
                  next.window.width = value;
                })
              }
            />
          </SettingRow>
          <SettingRow
            title={t("RememberWindowPosition")}
            description={t("RememberWindowPositionDescription")}
          >
            <Switch
              label={t("RememberWindowPosition")}
              checked={draft.window.remember_position}
              onCheckedChange={(value) =>
                update((next) => {
                  next.window.remember_position = value;
                  if (!value) next.window.position = null;
                })
              }
            />
          </SettingRow>
          <SettingRow title={t("BottomMargin")} description={t("BottomMarginDescription")}>
            <Slider
              label={t("BottomMargin")}
              value={draft.window.margin}
              min={0}
              max={500}
              unit=" px"
              onValueChange={(value) =>
                update((next) => {
                  next.window.margin = value;
                })
              }
            />
          </SettingRow>
          <SettingRow
            title={t("BottomPanelHeight")}
            description={t("BottomPanelHeightDescription")}
          >
            <Slider
              label={t("BottomPanelHeight")}
              value={draft.window.bottom_panel_height}
              min={0}
              max={200}
              unit=" px"
              onValueChange={(value) =>
                update((next) => {
                  next.window.bottom_panel_height = value;
                })
              }
            />
          </SettingRow>
          <SettingRow
            title={t("BackgroundOpacity")}
            description={t("BackgroundOpacityDescription")}
          >
            <Slider
              label={t("BackgroundOpacity")}
              value={draft.window.opacity}
              min={0.15}
              max={1}
              step={0.01}
              onValueChange={(value) =>
                update((next) => {
                  next.window.opacity = value;
                })
              }
            />
          </SettingRow>
        </Card>
      )}

      {subpage === "fonts" && (
        <Card>
          <SettingRow title={t("Fonts")} description={t("FontsDescription")}>
            <div className="font-picker-control">
              <Button variant="outline" onClick={() => sendUiAction({ type: "open-font-picker" })}>
                {t("ChangeFonts")}
              </Button>
              <span>{draft.lyrics.font_order.join(" → ")}</span>
            </div>
          </SettingRow>
          <SettingRow title={t("LyricFontSize")} description={t("LyricFontSizeDescription")}>
            <Slider
              label={t("LyricFontSize")}
              value={draft.lyrics.lyric_font_size}
              min={12}
              max={56}
              unit=" px"
              onValueChange={(value) =>
                update((next) => {
                  next.lyrics.lyric_font_size = value;
                })
              }
            />
          </SettingRow>
          <SettingRow
            title={t("TranslationFontSize")}
            description={t("TranslationFontSizeDescription")}
          >
            <Slider
              label={t("TranslationFontSize")}
              value={draft.lyrics.translation_font_size}
              min={8}
              max={36}
              unit=" px"
              onValueChange={(value) =>
                update((next) => {
                  next.lyrics.translation_font_size = value;
                })
              }
            />
          </SettingRow>
          <SettingRow
            title={t("RomanizationFontSize")}
            description={t("RomanizationFontSizeDescription")}
          >
            <Slider
              label={t("RomanizationFontSize")}
              value={draft.lyrics.romanization_font_size}
              min={8}
              max={36}
              unit=" px"
              onValueChange={(value) =>
                update((next) => {
                  next.lyrics.romanization_font_size = value;
                })
              }
            />
          </SettingRow>
        </Card>
      )}

      {subpage === "colors" && (
        <Card>
          {(
            [
              ["played_color", "PlayedColor", "PlayedColorDescription"],
              ["unplayed_color", "UnplayedColor", "UnplayedColorDescription"],
              ["translation_color", "TranslationColor", "TranslationColorDescription"],
              ["romanization_color", "RomanizationColor", "RomanizationColorDescription"],
            ] as const
          ).map(([field, title, description]) => (
            <SettingRow key={field} title={t(title)} description={t(description)}>
              <div className="color-control">
                <input
                  type="color"
                  value={draft.lyrics[field].slice(0, 7)}
                  onChange={(event) =>
                    update((next) => {
                      next.lyrics[field] = `${event.currentTarget.value.toUpperCase()}FF`;
                    })
                  }
                />
                <code>{draft.lyrics[field]}</code>
              </div>
            </SettingRow>
          ))}
        </Card>
      )}

      {subpage === "sources" && (
        <Card>
          <SettingRow title={t("SearchPriority")} description={t("SearchPriorityDescription")}>
            <Select
              value={draft.lyrics.provider_order.join(",")}
              onChange={(event) =>
                update((next) => {
                  next.lyrics.provider_order = event.currentTarget.value.split(
                    ",",
                  ) as AppConfig["lyrics"]["provider_order"];
                })
              }
            >
              <option value="qq-music,netease">{t("QqThenNetEase")}</option>
              <option value="netease,qq-music">{t("NetEaseThenQq")}</option>
            </Select>
          </SettingRow>
        </Card>
      )}
    </div>
  );
}

export function IntegrationPage({ state }: { state: UiState }) {
  const [draft, update] = useDraftConfig(state);
  if (!draft) return null;
  const t = (key: string) => text(state, key);

  return (
    <div className="page-stack">
      <div className="page-heading">
        <div>
          <h1>{t("Integration")}</h1>
          <p>{t("RunModeDescription")}</p>
        </div>
        <div className={state.saveError ? "save-state error" : "save-state"}>
          {state.saveError
            ? `${t("SaveFailed")}: ${state.saveError}`
            : state.saved
              ? t("Saved")
              : t("ChangesSavedAutomatically")}
        </div>
      </div>
      <Card>
        <SettingRow title={t("RunMode")} description={t("RunModeDescription")}>
          <Select
            value={draft.general.mode}
            onChange={(event) =>
              update((next) => {
                next.general.mode = event.currentTarget.value as AppConfig["general"]["mode"];
              })
            }
          >
            <option value="floating">{t("RunModeFloating")}</option>
            <option value="amll">{t("RunModeAmll")}</option>
          </Select>
        </SettingRow>
        <SettingRow title={t("AmllAddress")} description={t("AmllAddressDescription")}>
          <Input
            aria-label={t("AmllAddress")}
            value={draft.amll.address}
            placeholder="localhost:11444"
            disabled={draft.general.mode !== "amll"}
            onChange={(event) =>
              update((next) => {
                next.amll.address = event.currentTarget.value;
              })
            }
          />
        </SettingRow>
        <SettingRow title={t("TrayIcon")} description={t("TrayIconDescription")}>
          <Switch
            label={t("TrayIcon")}
            checked={draft.tray.enabled}
            onCheckedChange={(value) =>
              update((next) => {
                next.tray.enabled = value;
              })
            }
          />
        </SettingRow>
        <div className="card-intro">
          <p>{t("RunModeRestartHint")}</p>
        </div>
      </Card>
    </div>
  );
}

function formatDuration(duration: number | null): string {
  if (duration === null) return "—";
  const seconds = Math.round(duration / 1000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

function SearchPage({ state }: { state: UiState }) {
  const search: SearchState = state.search ?? {
    title: "",
    artist: "",
    status: text(state, "SearchAfterPlayback"),
    preview: text(state, "SelectCandidatePreview"),
    searching: false,
    applying: false,
    can_apply: false,
    selected_index: null,
    candidates: [],
  };
  const [title, setTitle] = useState(search.title);
  const [artist, setArtist] = useState(search.artist);
  useEffect(() => {
    setTitle(search.title);
    setArtist(search.artist);
  }, [search.title, search.artist]);
  const t = (key: string) => text(state, key);
  const submit = () => sendUiAction({ type: "search-lyrics", title, artist });
  return (
    <div className="page-stack search-page">
      <div className="page-heading">
        <div>
          <h1>{t("ManualSearchTitle")}</h1>
          <p>{search.status}</p>
        </div>
      </div>
      <Card className="search-card">
        <form
          className="search-form"
          onSubmit={(event) => {
            event.preventDefault();
            submit();
          }}
        >
          <label htmlFor="manual-search-title">
            {t("Title")}
            <Input
              id="manual-search-title"
              value={title}
              placeholder={t("SongTitle")}
              onChange={(event) => setTitle(event.currentTarget.value)}
            />
          </label>
          <label htmlFor="manual-search-artist">
            {t("Artist")}
            <Input
              id="manual-search-artist"
              value={artist}
              placeholder={t("Artist")}
              onChange={(event) => setArtist(event.currentTarget.value)}
            />
          </label>
          <Button type="submit" disabled={search.searching || search.applying}>
            {search.searching ? t("SearchingProviders") : t("Search")}
          </Button>
        </form>
        <div className="search-workspace">
          <div className="candidate-list">
            {search.candidates.map((candidate, index) => (
              <button
                type="button"
                className="candidate"
                data-selected={search.selected_index === index}
                key={`${candidate.provider}-${candidate.title}-${candidate.artists.join(";")}-${candidate.duration_ms ?? "unknown"}`}
                onClick={() => sendUiAction({ type: "preview-lyrics", index })}
              >
                <strong>{candidate.title}</strong>
                <span>{candidate.artists.join(" · ")}</span>
                <small>
                  {candidate.album || "—"} · {formatDuration(candidate.duration_ms)} ·{" "}
                  {t("MatchScore")} {candidate.match_score}
                </small>
              </button>
            ))}
          </div>
          <pre className="lyrics-preview">{search.preview}</pre>
        </div>
        <div className="search-footer">
          <span>{search.status}</span>
          <Button
            disabled={!search.can_apply || search.applying}
            onClick={() => sendUiAction({ type: "apply-lyrics" })}
          >
            {t("ApplySelectedLyrics")}
          </Button>
        </div>
      </Card>
    </div>
  );
}

function AboutPage({ state }: { state: UiState }) {
  const t = (key: string) => text(state, key);
  return (
    <div className="page-stack about-content">
      <Card className="hero-card">
        <AppIcon className="app-mark" />
        <h1>FloatLyrics</h1>
        <p>{t("AppSummary")}</p>
        <span>
          {t("Version")} {state.version}
        </span>
        <Button
          variant="outline"
          onClick={() =>
            sendUiAction({ type: "open-url", url: "https://github.com/ChouChiu/FloatLyrics" })
          }
        >
          {t("ProjectWebsite")}
        </Button>
      </Card>
      <Card>
        <div className="card-intro">
          <h2>{t("AcknowledgementsTitle")}</h2>
          <p>{t("InspiredByLyricsX")}</p>
          <Button
            variant="ghost"
            onClick={() =>
              sendUiAction({
                type: "open-url",
                url: "https://github.com/MxIris-LyricsX-Project/LyricsX",
              })
            }
          >
            {t("VisitLyricsX")}
          </Button>
        </div>
      </Card>
      <Card>
        <div className="card-intro">
          <h2>{t("OpenSourceTitle")}</h2>
          <p>{t("OpenSourceDescription")}</p>
        </div>
        <div className="dependency-grid">
          {state.about.dependencies.map((dependency) => (
            <div className="dependency" key={`${dependency.name}-${dependency.version}`}>
              <strong>{dependency.name}</strong>
              <span>
                v{dependency.version} · {dependency.license}
              </span>
            </div>
          ))}
        </div>
        <h3>{t("LicenseTexts")}</h3>
        {state.about.licenses.map((license) => (
          <details key={`${license.name}-${license.id}`}>
            <summary>
              {license.name} · {license.id}
            </summary>
            <pre>{license.text}</pre>
          </details>
        ))}
      </Card>
    </div>
  );
}

export function ControlCenter({ state }: { state: UiState }) {
  const t = (key: string) => text(state, key);
  const navigate = (page: ControlPage) => {
    sendUiAction({ type: "open-settings-page", page });
  };
  return (
    <div className="control-center">
      <aside className="sidebar">
        <div className="sidebar-title">FloatLyrics</div>
        <nav>
          {(
            [
              ["general", "settings", "General"],
              ["display", "display", "Display"],
              ["integration", "integration", "Integration"],
              ["sources", "sources", "LyricsSources"],
              ["about", "info", "About"],
            ] as const
          ).map(([page, icon, label]) => (
            <Button
              key={page}
              variant={state.page === page ? "default" : "ghost"}
              onClick={() => navigate(page)}
            >
              <Icon name={icon} />
              {t(label)}
            </Button>
          ))}
        </nav>
        <div className="sidebar-version">v{state.version}</div>
        <Button variant="ghost" onClick={() => sendUiAction({ type: "quit" })}>
          <Icon name="x" />
          {t("Quit")}
        </Button>
      </aside>
      <main className="control-content">
        {state.page === "about" ? (
          <AboutPage state={state} />
        ) : state.page === "integration" ? (
          <IntegrationPage key={state.page} state={state} />
        ) : (
          <SettingsPage key={state.page} state={state} page={state.page} />
        )}
      </main>
    </div>
  );
}

export function addFontFamily(fonts: string[], family: string): string[] {
  if (family.trim() === "" || fonts.includes(family)) return fonts;
  return [...fonts, family];
}

export function moveFontFamily(fonts: string[], index: number, delta: -1 | 1): string[] {
  const target = index + delta;
  if (index < 0 || index >= fonts.length || target < 0 || target >= fonts.length) return fonts;
  const next = [...fonts];
  const family = next[index];
  const targetFamily = next[target];
  if (family === undefined || targetFamily === undefined) return fonts;
  next[index] = targetFamily;
  next[target] = family;
  return next;
}

export function removeFontFamily(fonts: string[], index: number): string[] {
  if (fonts.length <= 1 || index < 0 || index >= fonts.length) return fonts;
  return fonts.filter((_, current) => current !== index);
}

export function FontPickerWindow({ state }: { state: UiState }) {
  const incoming = state.config?.lyrics.font_order ?? [];
  const [fonts, setFonts] = useState(incoming);
  useEffect(() => setFonts(incoming), [incoming]);
  if (!state.config) return null;
  const t = (key: string) => text(state, key);
  const persist = (next: string[]) => {
    if (next === fonts) return;
    setFonts(next);
    const config = cloneConfig(state.config as AppConfig);
    config.lyrics.font_order = next;
    sendUiAction({ type: "save-config", config });
  };

  return (
    <main className="font-picker-window">
      <div className="font-picker-columns">
        <section className="font-column">
          <h2>{t("AvailableFonts")}</h2>
          <div className="font-list available-fonts">
            {state.availableFonts.map((family) => (
              <button
                type="button"
                className="font-row available-font-row"
                data-selected={fonts.includes(family)}
                key={family}
                style={{ fontFamily: family }}
                onClick={() => persist(addFontFamily(fonts, family))}
              >
                {family}
              </button>
            ))}
          </div>
        </section>
        <section className="font-column">
          <h2>{t("FontOrder")}</h2>
          <div className="font-list selected-fonts">
            {fonts.map((family, index) => (
              <div className="font-row selected-font-row" key={family}>
                <span style={{ fontFamily: family }}>{family}</span>
                <div className="font-row-actions">
                  <Button
                    variant="ghost"
                    size="icon"
                    title={t("MoveFontUp")}
                    aria-label={t("MoveFontUp")}
                    disabled={index === 0}
                    onClick={() => persist(moveFontFamily(fonts, index, -1))}
                  >
                    <Icon name="up" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    title={t("MoveFontDown")}
                    aria-label={t("MoveFontDown")}
                    disabled={index + 1 === fonts.length}
                    onClick={() => persist(moveFontFamily(fonts, index, 1))}
                  >
                    <Icon name="down" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    title={t("RemoveFont")}
                    aria-label={t("RemoveFont")}
                    disabled={fonts.length <= 1}
                    onClick={() => persist(removeFontFamily(fonts, index))}
                  >
                    <Icon name="remove" />
                  </Button>
                </div>
              </div>
            ))}
          </div>
        </section>
      </div>
      <footer className="font-picker-footer">
        <Button onClick={() => sendUiAction({ type: "close-font-picker" })}>{t("Done")}</Button>
      </footer>
    </main>
  );
}

export function RootApp() {
  const state = useSyncExternalStore(uiStore.subscribe, uiStore.getSnapshot, uiStore.getSnapshot);
  if (state.surface === "overlay") return <OverlayShell state={state} />;
  if (state.surface === "control-center") return <ControlCenter state={state} />;
  if (state.surface === "manual-search") return <ManualSearchWindow state={state} />;
  if (state.surface === "font-picker") return <FontPickerWindow state={state} />;
  return null;
}

export function ManualSearchWindow({ state }: { state: UiState }) {
  return (
    <main className="manual-search-window">
      <SearchPage state={state} />
    </main>
  );
}
