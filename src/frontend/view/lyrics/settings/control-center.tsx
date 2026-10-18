// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import { Button, Icon } from "../components/ui";
import type { ControlPage } from "../types";
import { sendUiAction, type UiState } from "../ui-store";
import { translator } from "../ui-text";
import { AboutPage } from "./about";
import { SettingsPage } from "./pages";

const NAVIGATION = [
  ["general", "settings", "General"],
  ["display", "display", "Display"],
  ["sources", "sources", "LyricsSources"],
  ["integration", "integration", "Integration"],
  ["about", "info", "About"],
] as const satisfies ReadonlyArray<
  readonly [ControlPage, Parameters<typeof Icon>[0]["name"], string]
>;

export function ControlCenter({ state }: { state: UiState }) {
  const t = translator(state);
  return (
    <div className="control-center">
      <aside className="sidebar">
        <div className="sidebar-title">FloatLyrics</div>
        <nav aria-label={t("SettingsWindowTitle")}>
          {NAVIGATION.map(([page, icon, label]) => (
            <Button
              key={page}
              variant={state.page === page ? "default" : "ghost"}
              aria-current={state.page === page ? "page" : undefined}
              onClick={() => sendUiAction({ type: "open-settings-page", page })}
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
        ) : (
          <SettingsPage key={state.page} state={state} page={state.page} />
        )}
      </main>
    </div>
  );
}
