// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import type { ReactNode } from "react";
import { Button, Icon, Input, Select, SettingRow } from "../components/ui";
import { appendUnique, moveItem, removeItem } from "../list";
import { ALL_PROVIDERS, PROVIDER_NAMES } from "../providers";
import type { ControlPage } from "../types";
import { sendUiAction, type UiState } from "../ui-store";
import { translator } from "../ui-text";
import { useDraftConfig } from "./draft";
import {
  ColorSetting,
  type FieldContext,
  NumberSetting,
  PageHeading,
  Section,
  SwitchSetting,
} from "./fields";

function GeneralSettings({ ctx }: { ctx: FieldContext }) {
  const { draft, update, t } = ctx;
  return (
    <>
      <Section title={t("Language")}>
        <SettingRow title={t("Language")} description={t("LanguageDescription")}>
          <Select
            label={t("Language")}
            value={draft.general.language}
            options={[
              { value: "en", label: "English" },
              { value: "zh-CN", label: "简体中文" },
              { value: "zh-TW", label: "繁體中文" },
            ]}
            onValueChange={(value) =>
              update((next) => {
                next.general.language = value;
              })
            }
          />
        </SettingRow>
      </Section>
      <Section title={t("Timing")} description={t("TimingDescription")}>
        <NumberSetting
          ctx={ctx}
          path="lyrics.offset_ms"
          title={t("GlobalOffset")}
          description={t("GlobalOffsetDescription")}
          step={50}
          unit=" ms"
        />
        <NumberSetting
          ctx={ctx}
          path="lyrics.track_offset_step_ms"
          title={t("TrackOffsetStep")}
          description={t("TrackOffsetStepDescription")}
          step={10}
          unit=" ms"
        />
      </Section>
      <Section title={t("LyricsContent")}>
        <SwitchSetting
          title={t("ShowTranslation")}
          description={t("ShowTranslationDescription")}
          checked={draft.lyrics.show_translation}
          onChange={(value) =>
            update((next) => {
              next.lyrics.show_translation = value;
            })
          }
        />
        <SwitchSetting
          title={t("ShowRomanization")}
          description={t("ShowRomanizationDescription")}
          checked={draft.lyrics.show_romanization}
          onChange={(value) =>
            update((next) => {
              next.lyrics.show_romanization = value;
            })
          }
        />
        <SettingRow
          title={t("ChineseRomanization")}
          description={t("ChineseRomanizationDescription")}
          disabled={!draft.lyrics.show_romanization}
        >
          <Select
            label={t("ChineseRomanization")}
            disabled={!draft.lyrics.show_romanization}
            value={draft.lyrics.chinese_romanization}
            options={[
              { value: "auto", label: t("RomanizationAutomatic") },
              { value: "mandarin-pinyin", label: t("MandarinPinyin") },
              { value: "cantonese-jyutping", label: t("CantoneseJyutping") },
              { value: "cantonese-jyutping-no-tones", label: t("CantoneseJyutpingWithoutTones") },
            ]}
            onValueChange={(value) =>
              update((next) => {
                next.lyrics.chinese_romanization = value;
              })
            }
          />
        </SettingRow>
      </Section>
    </>
  );
}

function DisplaySettings({ ctx }: { ctx: FieldContext }) {
  const { draft, update, t } = ctx;
  return (
    <>
      <Section title={t("Panel")}>
        <SwitchSetting
          title={t("AppleMusicStyle")}
          description={t("AppleMusicStyleDescription")}
          checked={draft.lyrics.apple_music_style}
          onChange={(value) =>
            update((next) => {
              next.lyrics.apple_music_style = value;
            })
          }
        />
        <NumberSetting
          ctx={ctx}
          path="window.width"
          title={t("PanelWidth")}
          description={t("PanelWidthDescription")}
          step={10}
          unit=" px"
        />
        <NumberSetting
          ctx={ctx}
          path="window.opacity"
          title={t("BackgroundOpacity")}
          description={t("BackgroundOpacityDescription")}
          step={0.01}
        />
      </Section>
      <Section title={t("Position")}>
        <SwitchSetting
          title={t("RememberWindowPosition")}
          description={t("RememberWindowPositionDescription")}
          checked={draft.window.remember_position}
          onChange={(value) =>
            update((next) => {
              next.window.remember_position = value;
              if (!value) next.window.position = null;
            })
          }
        />
        <NumberSetting
          ctx={ctx}
          path="window.margin"
          title={t("BottomMargin")}
          description={t("BottomMarginDescription")}
          unit=" px"
        />
        <NumberSetting
          ctx={ctx}
          path="window.bottom_panel_height"
          title={t("BottomPanelHeight")}
          description={t("BottomPanelHeightDescription")}
          unit=" px"
        />
      </Section>
      <Section title={t("Fonts")}>
        <SettingRow title={t("FontFamilies")} description={t("FontsDescription")}>
          <div className="font-picker-control">
            <Button variant="outline" onClick={() => sendUiAction({ type: "open-font-picker" })}>
              {t("ChangeFonts")}
            </Button>
            <span title={draft.lyrics.font_order.join(" → ")}>
              {draft.lyrics.font_order.join(" → ")}
            </span>
          </div>
        </SettingRow>
        <NumberSetting
          ctx={ctx}
          path="lyrics.lyric_font_size"
          title={t("LyricFontSize")}
          unit=" px"
        />
        <NumberSetting
          ctx={ctx}
          path="lyrics.translation_font_size"
          title={t("TranslationFontSize")}
          unit=" px"
        />
        <NumberSetting
          ctx={ctx}
          path="lyrics.romanization_font_size"
          title={t("RomanizationFontSize")}
          unit=" px"
        />
      </Section>
      <Section title={t("Colors")} description={t("ColorsDescription")}>
        <ColorSetting ctx={ctx} field="played_color" title={t("PlayedColor")} />
        <ColorSetting ctx={ctx} field="unplayed_color" title={t("UnplayedColor")} />
        <ColorSetting ctx={ctx} field="translation_color" title={t("TranslationColor")} />
        <ColorSetting ctx={ctx} field="romanization_color" title={t("RomanizationColor")} />
      </Section>
      <Section title={t("Animation")}>
        <NumberSetting
          ctx={ctx}
          path="lyrics.transition_ms"
          title={t("LineTransition")}
          description={t("LineTransitionDescription")}
          step={10}
          unit=" ms"
        />
        <NumberSetting
          ctx={ctx}
          path="lyrics.word_fade_width"
          title={t("WordFadeWidth")}
          description={t("WordFadeWidthDescription")}
          step={0.05}
          unit=" em"
        />
      </Section>
    </>
  );
}

function SourceSettings({ ctx }: { ctx: FieldContext }) {
  const { draft, update, t } = ctx;
  const order = draft.lyrics.provider_order;
  const available = ALL_PROVIDERS.filter((provider) => !order.includes(provider));
  const edit = (change: (providers: typeof order) => typeof order) =>
    update((next) => {
      next.lyrics.provider_order = change(next.lyrics.provider_order);
    });

  return (
    <Section title={t("SearchPriority")} description={t("SearchPriorityDescription")}>
      <div className="source-control">
        <ol className="source-list">
          {order.map((provider, index) => (
            <li className="source-row" key={provider}>
              <span className="source-rank">{index + 1}</span>
              <span className="source-name">{t(PROVIDER_NAMES[provider])}</span>
              <div className="row-actions">
                <Button
                  variant="ghost"
                  size="icon"
                  title={t("MoveSourceUp")}
                  aria-label={t("MoveSourceUp")}
                  disabled={index === 0}
                  onClick={() => edit((providers) => moveItem(providers, index, -1))}
                >
                  <Icon name="up" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  title={t("MoveSourceDown")}
                  aria-label={t("MoveSourceDown")}
                  disabled={index + 1 === order.length}
                  onClick={() => edit((providers) => moveItem(providers, index, 1))}
                >
                  <Icon name="down" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  title={t("RemoveSource")}
                  aria-label={t("RemoveSource")}
                  disabled={order.length <= 1}
                  onClick={() => edit((providers) => removeItem(providers, index))}
                >
                  <Icon name="remove" />
                </Button>
              </div>
            </li>
          ))}
        </ol>
        {available.length > 0 && (
          <div className="source-available">
            <div className="source-list-title">{t("AvailableSources")}</div>
            <div className="source-available-rows">
              {available.map((provider) => (
                <Button
                  key={provider}
                  variant="outline"
                  size="sm"
                  onClick={() => edit((providers) => appendUnique(providers, provider))}
                >
                  <Icon name="plus" />
                  <span>{t(PROVIDER_NAMES[provider])}</span>
                </Button>
              ))}
            </div>
          </div>
        )}
      </div>
    </Section>
  );
}

function IntegrationSettings({ ctx }: { ctx: FieldContext }) {
  const { draft, update, t } = ctx;
  return (
    <>
      <Section title={t("RunMode")}>
        <SettingRow title={t("RunMode")} description={t("RunModeDescription")}>
          <Select
            label={t("RunMode")}
            value={draft.general.mode}
            options={[
              { value: "floating", label: t("RunModeFloating") },
              { value: "amll", label: t("RunModeAmll") },
            ]}
            onValueChange={(value) =>
              update((next) => {
                next.general.mode = value;
              })
            }
          />
        </SettingRow>
        <SettingRow
          title={t("AmllAddress")}
          description={t("AmllAddressDescription")}
          disabled={draft.general.mode !== "amll"}
        >
          <Input
            aria-label={t("AmllAddress")}
            value={draft.amll.address}
            placeholder={ctx.state.defaults?.amll.address}
            disabled={draft.general.mode !== "amll"}
            onChange={(event) =>
              update((next) => {
                next.amll.address = event.currentTarget.value;
              })
            }
          />
        </SettingRow>
      </Section>
      <Section title={t("System")}>
        <SwitchSetting
          title={t("TrayIcon")}
          description={t("TrayIconDescription")}
          checked={draft.tray.enabled}
          onChange={(value) =>
            update((next) => {
              next.tray.enabled = value;
            })
          }
        />
      </Section>
    </>
  );
}

export type SettingsPageName = Exclude<ControlPage, "about">;

const PAGES: Record<
  SettingsPageName,
  { title: string; description: string; body(props: { ctx: FieldContext }): ReactNode }
> = {
  general: { title: "General", description: "GeneralDescription", body: GeneralSettings },
  display: { title: "Display", description: "DisplayDescription", body: DisplaySettings },
  integration: {
    title: "Integration",
    description: "IntegrationDescription",
    body: IntegrationSettings,
  },
  sources: { title: "LyricsSources", description: "SourcesDescription", body: SourceSettings },
};

export function SettingsPage({ state, page }: { state: UiState; page: SettingsPageName }) {
  const [draft, update] = useDraftConfig(state);
  if (!draft) return null;
  const t = translator(state);
  const { title, description, body: Body } = PAGES[page];

  return (
    <div className="page-stack">
      <PageHeading title={t(title)} description={t(description)} state={state} t={t} />
      <Body ctx={{ state, draft, update, t }} />
    </div>
  );
}
