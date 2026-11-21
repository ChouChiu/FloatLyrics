// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { SearchState } from "../types";
import { advanceUiState, uiStore } from "../ui-store";
import { ManualSearchWindow } from "./page";

const search: SearchState = {
  track: null,
  query_revision: 0,
  title: "",
  artist: "",
  status: "Start playback to search for lyrics.",
  preview: "",
  searching: false,
  applying: false,
  can_apply: false,
  selected_index: null,
  candidates: [],
};

function render(next: Partial<SearchState>) {
  const state = advanceUiState(uiStore.getSnapshot(), {
    type: "search-state",
    state: { ...search, ...next },
  });
  return renderToStaticMarkup(<ManualSearchWindow state={state} />);
}

test("disables both forms without a track and announces why", () => {
  const html = render({});
  expect(html).toContain('class="manual-search-window"');
  expect(html).not.toContain('class="sidebar"');
  expect(html).toMatch(/<input[^>]*id="manual-search-title"[^>]*disabled/);
  expect(html).toMatch(/<input[^>]*id="manual-search-qq-reference"[^>]*disabled/);
  expect(html).toContain('role="status">Start playback to search for lyrics.');
});

test("exposes the selected result and makes the preview keyboard-scrollable", () => {
  const html = render({
    track: "Song · Artist",
    title: "Song",
    selected_index: 0,
    can_apply: true,
    candidates: [
      {
        provider: "qq-music",
        provider_track_id: "song",
        title: "Song",
        artists: ["Artist"],
        album: "",
        duration_ms: null,
        match_score: 100,
      },
    ],
  });
  expect(html).toContain("Song · Artist");
  expect(html).toContain('aria-pressed="true"');
  expect(html).toMatch(/<textarea[^>]*aria-labelledby="lyrics-preview-heading"[^>]*readOnly/i);
  expect(html).toMatch(/<button(?![^>]*disabled)[^>]*>ApplySelectedLyrics/);
});
