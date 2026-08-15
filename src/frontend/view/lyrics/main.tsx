// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import { createRoot } from "react-dom/client";
import "@applemusic-like-lyrics/core/style.css";
import { installLyricsBridge } from "./bridge";
import { RootApp } from "./shell";
import { lyricsStore } from "./store";
import { uiStore } from "./ui-store";

installLyricsBridge(window, (command) => {
  lyricsStore.dispatch(command);
  uiStore.dispatch(command);
});

const root = document.getElementById("root");
if (!root) throw new Error("missing React lyrics root");
createRoot(root).render(<RootApp />);
