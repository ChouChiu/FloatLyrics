// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import type { UiState } from "./ui-store";

/** Translator bound to the catalogue the backend sent; a missing key shows itself. */
export type Translate = (key: string, values?: Record<string, string | number>) => string;

export function translator(state: UiState): Translate {
  return (key, values) => {
    let message = state.strings[key] ?? key;
    if (values) {
      for (const [name, value] of Object.entries(values)) {
        message = message.replaceAll(`{${name}}`, String(value));
      }
    }
    return message;
  };
}
