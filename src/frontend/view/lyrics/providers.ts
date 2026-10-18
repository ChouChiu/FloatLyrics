// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import type { LyricsProvider } from "./types";

/** Text key of each lyrics source's display name, in the order the settings offer them. */
export const PROVIDER_NAMES: Record<LyricsProvider, string> = {
  "amll-ttml-db": "ProviderNameAmllTtmlDb",
  "qq-music": "ProviderNameQqMusic",
  netease: "ProviderNameNetEase",
  kugou: "ProviderNameKugou",
  lrclib: "ProviderNameLrclib",
  "soda-music": "ProviderNameSodaMusic",
};

export const ALL_PROVIDERS = Object.keys(PROVIDER_NAMES) as LyricsProvider[];
