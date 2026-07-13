// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: GPL-3.0-or-later

import { createRoot } from "react-dom/client";
import { LyricsApp } from "./app";
import { installLyricsBridge } from "./bridge";
import { lyricsStore } from "./store";

installLyricsBridge(window, lyricsStore.render);

const root = document.getElementById("root");
if (!root) throw new Error("missing React lyrics root");
createRoot(root).render(<LyricsApp />);
