// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import { useSyncExternalStore } from "react";
import { FontPickerWindow } from "./font-picker";
import { OverlayShell } from "./overlay";
import { ManualSearchWindow } from "./search";
import { ControlCenter } from "./settings/control-center";
import { uiStore } from "./ui-store";

/** Renders the surface the backend bootstrapped this page as. */
export function RootApp() {
  const state = useSyncExternalStore(uiStore.subscribe, uiStore.getSnapshot, uiStore.getSnapshot);
  switch (state.surface) {
    case "overlay":
      return <OverlayShell state={state} />;
    case "control-center":
      return <ControlCenter state={state} />;
    case "manual-search":
      return <ManualSearchWindow state={state} />;
    case "font-picker":
      return <FontPickerWindow state={state} />;
    default:
      return null;
  }
}
