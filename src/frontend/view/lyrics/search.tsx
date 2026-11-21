// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import { useEffect, useState } from "react";
import { Button, Card, Input } from "./components/ui";
import { PROVIDER_NAMES } from "./providers";
import type { SearchState } from "./types";
import { sendUiAction, type UiState } from "./ui-store";
import { translator } from "./ui-text";

function formatDuration(duration: number | null): string {
  if (duration === null) return "—";
  const seconds = Math.round(duration / 1000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

export function SearchPage({ state }: { state: UiState }) {
  const t = translator(state);
  const search: SearchState = state.search ?? {
    title: "",
    artist: "",
    status: t("SearchAfterPlayback"),
    preview: t("SelectCandidatePreview"),
    searching: false,
    applying: false,
    can_apply: false,
    selected_index: null,
    candidates: [],
  };
  const [title, setTitle] = useState(search.title);
  const [artist, setArtist] = useState(search.artist);
  const [reference, setReference] = useState("");
  useEffect(() => {
    setTitle(search.title);
    setArtist(search.artist);
  }, [search.title, search.artist]);
  const submit = () => sendUiAction({ type: "search-lyrics", title, artist });
  return (
    <div className="page-stack search-page">
      <div className="page-heading">
        <div>
          <h1>{t("ManualSearchTitle")}</h1>
          <p>{t("ManualSearchDescription")}</p>
        </div>
      </div>
      <Card className="search-card">
        <form
          className="search-form"
          onSubmit={(event) => {
            event.preventDefault();
            submit();
          }}
        >
          <label htmlFor="manual-search-title">
            {t("Title")}
            <Input
              id="manual-search-title"
              value={title}
              placeholder={t("SongTitle")}
              onChange={(event) => setTitle(event.currentTarget.value)}
            />
          </label>
          <label htmlFor="manual-search-artist">
            {t("Artist")}
            <Input
              id="manual-search-artist"
              value={artist}
              placeholder={t("Artist")}
              onChange={(event) => setArtist(event.currentTarget.value)}
            />
          </label>
          <Button type="submit" disabled={search.searching || search.applying}>
            {search.searching ? t("SearchingProviders") : t("Search")}
          </Button>
        </form>
        <form
          className="search-form search-direct-form"
          onSubmit={(event) => {
            event.preventDefault();
            sendUiAction({ type: "lookup-qq-lyrics", reference });
          }}
        >
          <label htmlFor="manual-search-qq-reference">
            {t("QqSongReference")}
            <Input
              id="manual-search-qq-reference"
              value={reference}
              onChange={(event) => setReference(event.currentTarget.value)}
            />
          </label>
          <Button type="submit" disabled={search.applying || !reference.trim()}>
            {t("LookupQqLyrics")}
          </Button>
        </form>
        <div className="search-workspace">
          <div className="candidate-list">
            {search.candidates.map((candidate, index) => (
              <button
                type="button"
                className="candidate"
                data-selected={search.selected_index === index}
                key={`${candidate.provider}-${candidate.title}-${candidate.artists.join(";")}-${candidate.duration_ms ?? "unknown"}`}
                onClick={() => sendUiAction({ type: "preview-lyrics", index })}
              >
                <strong>{candidate.title}</strong>
                <span>{candidate.artists.join(" · ")}</span>
                <small>
                  {t(PROVIDER_NAMES[candidate.provider])} · {candidate.album || "—"} ·{" "}
                  {formatDuration(candidate.duration_ms)} · {t("MatchScore")}{" "}
                  {candidate.match_score}
                </small>
              </button>
            ))}
          </div>
          {/* Remounted per candidate so a new preview starts at its first line. */}
          <pre className="lyrics-preview" key={search.selected_index ?? "none"}>
            {search.preview}
          </pre>
        </div>
        <div className="search-footer">
          <span>{search.status}</span>
          <Button
            disabled={!search.can_apply || search.applying}
            onClick={() => sendUiAction({ type: "apply-lyrics" })}
          >
            {t("ApplySelectedLyrics")}
          </Button>
        </div>
      </Card>
    </div>
  );
}

export function ManualSearchWindow({ state }: { state: UiState }) {
  return (
    <main className="manual-search-window">
      <SearchPage state={state} />
    </main>
  );
}
