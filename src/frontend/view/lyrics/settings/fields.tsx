// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import type { ReactNode } from "react";
import { Button, Card, Icon, SettingRow, Slider, Switch } from "../components/ui";
import type { AppConfig } from "../types";
import type { UiState } from "../ui-store";
import type { Translate } from "../ui-text";
import { type ConfigUpdate, type NumericPath, readNumber, writeNumber } from "./draft";

/** What every settings field needs to read, describe, and save itself. */
export interface FieldContext {
  state: UiState;
  draft: AppConfig;
  update: ConfigUpdate;
  t: Translate;
}

function SaveStatus({ state, t }: { state: UiState; t: Translate }) {
  if (state.saveError) {
    return (
      <div className="save-state error" role="alert">
        {t("SaveFailed")}
        {t("DetailSeparator")}
        {state.saveError}
      </div>
    );
  }
  return (
    <div className="save-state" role="status">
      {t(state.saving ? "SavingSettings" : state.saved ? "Saved" : "ChangesSavedAutomatically")}
    </div>
  );
}

export function PageHeading({
  title,
  description,
  state,
  t,
}: {
  title: string;
  description: string;
  state: UiState;
  t: Translate;
}) {
  return (
    <div className="page-heading">
      <div>
        <h1>{title}</h1>
        <p>{description}</p>
      </div>
      <SaveStatus state={state} t={t} />
    </div>
  );
}

/** A titled group of settings rows. */
export function Section({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <section className="settings-section">
      <header className="settings-section-heading">
        <h2>{title}</h2>
        {description && <p>{description}</p>}
      </header>
      <Card>{children}</Card>
    </section>
  );
}

function ResetButton({ visible, onReset, t }: { visible: boolean; onReset(): void; t: Translate }) {
  return (
    <Button
      variant="ghost"
      size="icon"
      className="reset-button"
      title={t("ResetToDefault")}
      aria-label={t("ResetToDefault")}
      aria-hidden={!visible}
      tabIndex={visible ? 0 : -1}
      data-visible={visible}
      onClick={onReset}
    >
      <Icon name="reset" />
    </Button>
  );
}

/**
 * A slider bound to one numeric preference.
 *
 * The range comes from the limit table the backend validates against, and a
 * reset button appears once the value differs from its default.
 */
export function NumberSetting({
  ctx,
  path,
  title,
  description,
  step = 1,
  unit,
  disabled,
}: {
  ctx: FieldContext;
  path: NumericPath;
  title: string;
  description?: string;
  step?: number;
  unit?: string;
  disabled?: boolean;
}) {
  const range = ctx.state.limits[path];
  if (!range) return null;
  const value = readNumber(ctx.draft, path);
  const fallback = ctx.state.defaults ? readNumber(ctx.state.defaults, path) : value;
  const set = (next: number) => ctx.update((config) => writeNumber(config, path, next));

  return (
    <SettingRow title={title} description={description} disabled={disabled}>
      <div className="number-setting">
        <Slider
          label={title}
          value={value}
          min={range.min}
          max={range.max}
          step={step}
          unit={unit}
          onValueChange={set}
        />
        <ResetButton visible={value !== fallback} onReset={() => set(fallback)} t={ctx.t} />
      </div>
    </SettingRow>
  );
}

export function SwitchSetting({
  title,
  description,
  checked,
  disabled,
  onChange,
}: {
  title: string;
  description?: string;
  checked: boolean;
  disabled?: boolean;
  onChange(value: boolean): void;
}) {
  return (
    <SettingRow title={title} description={description} disabled={disabled}>
      <Switch label={title} checked={checked} disabled={disabled} onCheckedChange={onChange} />
    </SettingRow>
  );
}

type ColorField = "played_color" | "unplayed_color" | "translation_color" | "romanization_color";

/** Splits `#RRGGBBAA` into the `#rrggbb` a color input takes and an alpha percentage. */
export function splitColor(value: string): { rgb: string; alpha: number } {
  const hex = value.replace(/^#/, "");
  const alphaByte = hex.length >= 8 ? Number.parseInt(hex.slice(6, 8), 16) : 255;
  return {
    rgb: `#${hex.slice(0, 6).toLowerCase()}`,
    alpha: Math.round(((Number.isNaN(alphaByte) ? 255 : alphaByte) / 255) * 100),
  };
}

/** Joins a `#rrggbb` color and an alpha percentage into `#RRGGBBAA`. */
export function joinColor(rgb: string, alpha: number): string {
  const byte = Math.round((Math.min(100, Math.max(0, alpha)) / 100) * 255);
  return `${rgb.toUpperCase()}${byte.toString(16).padStart(2, "0").toUpperCase()}`;
}

export function ColorSetting({
  ctx,
  field,
  title,
  description,
}: {
  ctx: FieldContext;
  field: ColorField;
  title: string;
  description?: string;
}) {
  const value = ctx.draft.lyrics[field];
  const fallback = ctx.state.defaults?.lyrics[field] ?? value;
  const { rgb, alpha } = splitColor(value);
  const set = (next: string) =>
    ctx.update((config) => {
      config.lyrics[field] = next;
    });

  return (
    <SettingRow title={title} description={description}>
      <div className="color-control">
        <input
          type="color"
          aria-label={title}
          value={rgb}
          onChange={(event) => set(joinColor(event.currentTarget.value, alpha))}
        />
        <Slider
          className="compact"
          label={`${title} ${ctx.t("Opacity")}`}
          value={alpha}
          min={0}
          max={100}
          unit="%"
          onValueChange={(next) => set(joinColor(rgb, next))}
        />
        <ResetButton
          visible={value.toUpperCase() !== fallback.toUpperCase()}
          onReset={() => set(fallback)}
          t={ctx.t}
        />
      </div>
    </SettingRow>
  );
}
