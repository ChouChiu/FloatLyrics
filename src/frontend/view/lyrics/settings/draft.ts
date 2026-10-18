// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import { useEffect, useState } from "react";
import type { AppConfig } from "../types";
import { sendUiAction, type UiState } from "../ui-store";

/** Applies one edit to a copy of the configuration. */
export type ConfigUpdate = (change: (next: AppConfig) => void) => void;

/**
 * Draft of the configuration a page edits.
 *
 * Every edit is saved at once, so the draft follows the configuration the
 * backend reports back and is `null` until the first state arrives.
 */
export function useDraftConfig(state: UiState): [AppConfig | null, ConfigUpdate] {
  const incoming = state.config;
  const [draft, setDraft] = useState<AppConfig | null>(incoming);
  useEffect(() => setDraft(incoming), [incoming]);

  const update: ConfigUpdate = (change) => {
    if (!draft) return;
    const next = structuredClone(draft);
    change(next);
    setDraft(next);
    sendUiAction({ type: "save-config", config: next });
  };
  return [draft, update];
}

type NumericKey<S extends keyof AppConfig> = {
  [K in keyof AppConfig[S]]: AppConfig[S][K] extends number ? K : never;
}[keyof AppConfig[S]] &
  string;

/** Dotted path of a numeric preference, matching the backend's limit table. */
export type NumericPath = `window.${NumericKey<"window">}` | `lyrics.${NumericKey<"lyrics">}`;

function split(path: NumericPath): [keyof AppConfig, string] {
  const [section, key] = path.split(".") as [keyof AppConfig, string];
  return [section, key];
}

export function readNumber(config: AppConfig, path: NumericPath): number {
  const [section, key] = split(path);
  return (config[section] as unknown as Record<string, number>)[key] ?? 0;
}

export function writeNumber(config: AppConfig, path: NumericPath, value: number): void {
  const [section, key] = split(path);
  (config[section] as unknown as Record<string, number>)[key] = value;
}
