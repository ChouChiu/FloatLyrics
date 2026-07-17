// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import type { FloatLyricsBridge, LyricsCommand } from "./types";

interface BridgeHost {
  floatLyrics?: FloatLyricsBridge;
  floatLyricsPendingCommands?: LyricsCommand[];
}

export function installLyricsBridge(
  host: BridgeHost,
  dispatch: (command: LyricsCommand) => void,
): FloatLyricsBridge {
  const pendingCommands = host.floatLyricsPendingCommands ?? [];
  const bridge = Object.freeze({ dispatch });
  host.floatLyrics = bridge;
  delete host.floatLyricsPendingCommands;
  for (const command of pendingCommands) dispatch(command);
  return bridge;
}
