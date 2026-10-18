// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import type { CSSProperties } from "react";
import { LyricsApp } from "./app";
import { Button, Icon } from "./components/ui";
import { sendUiAction, type UiState } from "./ui-store";
import { translator } from "./ui-text";

const DEFAULT_OFFSET_STEP_MS = 100;

export function OverlayShell({ state }: { state: UiState }) {
  const t = translator(state);
  const step = state.config?.lyrics.track_offset_step_ms ?? DEFAULT_OFFSET_STEP_MS;

  return (
    <div
      className={["overlay-shell", ...state.snapClasses].join(" ")}
      style={{ "--panel-opacity": state.panelOpacity } as CSSProperties}
    >
      <header className="overlay-toolbar">
        <div className="track-title" title={state.songInfo}>
          {state.songInfo}
        </div>
        <div className="offset-control">
          <Button
            variant="ghost"
            size="icon"
            title={t("DecreaseTrackOffsetTooltip", { step })}
            aria-label={t("DecreaseTrackOffsetTooltip", { step })}
            onClick={() => sendUiAction({ type: "adjust-track-offset", delta_ms: -step })}
          >
            <Icon name="minus" />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            title={t("ResetTrackOffsetTooltip")}
            onClick={() => sendUiAction({ type: "reset-track-offset" })}
          >
            {state.trackOffset}
          </Button>
          <Button
            variant="ghost"
            size="icon"
            title={t("IncreaseTrackOffsetTooltip", { step })}
            aria-label={t("IncreaseTrackOffsetTooltip", { step })}
            onClick={() => sendUiAction({ type: "adjust-track-offset", delta_ms: step })}
          >
            <Icon name="plus" />
          </Button>
        </div>
        {(
          [
            ["ManualSearchTooltip", "search", { type: "open-search" }],
            ["OpenSettingsTooltip", "settings", { type: "open-settings" }],
            ["CloseTooltip", "x", { type: "quit" }],
          ] as const
        ).map(([label, icon, action]) => (
          <Button
            key={label}
            variant="ghost"
            size="icon"
            title={t(label)}
            aria-label={t(label)}
            onClick={() => sendUiAction(action)}
          >
            <Icon name={icon} />
          </Button>
        ))}
      </header>
      <div className="overlay-divider" />
      <div className="lyrics-region">
        <LyricsApp />
      </div>
    </div>
  );
}
