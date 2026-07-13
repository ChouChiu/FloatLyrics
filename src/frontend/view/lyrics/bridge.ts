// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

import type { FloatLyricsBridge, LyricsPayload } from "./types";

interface BridgeHost {
  floatLyrics?: FloatLyricsBridge;
  floatLyricsPendingPayload?: LyricsPayload;
}

export function installLyricsBridge(
  host: BridgeHost,
  render: (payload: LyricsPayload) => void,
): FloatLyricsBridge {
  const pendingPayload = host.floatLyricsPendingPayload;
  const bridge = Object.freeze({ render });
  host.floatLyrics = bridge;
  delete host.floatLyricsPendingPayload;
  if (pendingPayload) render(pendingPayload);
  return bridge;
}
