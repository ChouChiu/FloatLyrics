// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import { createRoot } from "react-dom/client";
import "@applemusic-like-lyrics/core/style.css";
import { LyricsApp } from "./app";
import { installLyricsBridge } from "./bridge";
import { lyricsStore } from "./store";

installLyricsBridge(window, lyricsStore.dispatch);

const root = document.getElementById("root");
if (!root) throw new Error("missing React lyrics root");
createRoot(root).render(<LyricsApp />);
