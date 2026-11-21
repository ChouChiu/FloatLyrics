// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import { useState } from "react";
import { Button, Card, Input } from "../components/ui";
import { PROVIDER_NAMES } from "../providers";
import type { SearchState } from "../types";
import { sendUiAction, type UiState } from "../ui-store";
import { type Translate, translator } from "../ui-text";
import "./search.css";

function formatDuration(duration: number): string {
  const seconds = Math.round(duration / 1000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

function SearchForms({ search, t }: { search: SearchState; t: Translate }) {
  const [title, setTitle] = useState(search.title);
  const [artist, setArtist] = useState(search.artist);
  const [reference, setReference] = useState("");
  const busy = search.searching || search.applying;
  const unavailable = !search.track || search.applying;

  return (
    <>
      <form
        className="search-form"
        onSubmit={(event) => {
          event.preventDefault();
          if (!unavailable && !busy && title.trim()) {
            sendUiAction({ type: "search-lyrics", title, artist });
          }
        }}
      >
        <label htmlFor="manual-search-title">
          {t("Title")}
          <Input
            id="manual-search-title"
            value={title}
            disabled={unavailable}
            required
            onChange={(event) => setTitle(event.currentTarget.value)}
          />
        </label>
        <label htmlFor="manual-search-artist">
          {t("Artist")}
          <Input
            id="manual-search-artist"
            value={artist}
            disabled={unavailable}
            onChange={(event) => setArtist(event.currentTarget.value)}
          />
        </label>
        <Button type="submit" disabled={unavailable || busy || !title.trim()}>
          {t("Search")}
        </Button>
      </form>
      <form
        className="search-form search-direct-form"
        onSubmit={(event) => {
          event.preventDefault();
          if (!unavailable && !busy && reference.trim()) {
            sendUiAction({ type: "lookup-qq-lyrics", reference });
          }
        }}
      >
        <label htmlFor="manual-search-qq-reference">
          {t("QqSongReference")}
          <Input
            id="manual-search-qq-reference"
            value={reference}
            disabled={unavailable}
            onChange={(event) => setReference(event.currentTarget.value)}
          />
        </label>
        <Button type="submit" variant="outline" disabled={unavailable || busy || !reference.trim()}>
          {t("LookupQqLyrics")}
        </Button>
      </form>
    </>
  );
}

function SearchResults({ search, t }: { search: SearchState; t: Translate }) {
  return (
    <div className="search-workspace">
      <section className="search-results" aria-busy={search.searching}>
        <h2 id="search-results-heading">{t("SearchResults")}</h2>
        <ul className="candidate-list" aria-labelledby="search-results-heading">
          {search.candidates.map((candidate, index) => (
            <li key={`${candidate.provider}-${candidate.provider_track_id}`}>
              <button
                type="button"
                className="candidate"
                aria-pressed={search.selected_index === index}
                disabled={search.applying}
                onClick={() => sendUiAction({ type: "preview-lyrics", index })}
              >
                <strong>{candidate.title}</strong>
                <span>{candidate.artists.join(" · ")}</span>
                <small>
                  {[
                    t(PROVIDER_NAMES[candidate.provider]),
                    candidate.album,
                    candidate.duration_ms !== null && formatDuration(candidate.duration_ms),
                    `${t("MatchScore")} ${candidate.match_score}`,
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                </small>
              </button>
            </li>
          ))}
        </ul>
      </section>
      <section className="search-results">
        <h2 id="lyrics-preview-heading">{t("LyricsPreview")}</h2>
        <textarea
          className="lyrics-preview"
          key={search.selected_index ?? "none"}
          aria-labelledby="lyrics-preview-heading"
          value={search.preview}
          readOnly
        />
      </section>
    </div>
  );
}

export function ManualSearchWindow({ state }: { state: UiState }) {
  const t = translator(state);
  const { search } = state;
  if (!search) return null;
  return (
    <main className="manual-search-window">
      <div className="page-stack search-page">
        <header className="page-heading">
          <div>
            <h1>{t("ManualSearchTitle")}</h1>
            <p>{t("ManualSearchDescription")}</p>
            {search.track && (
              <p className="search-track">
                {t("CurrentTrack")} <strong>{search.track}</strong>
              </p>
            )}
          </div>
          <Button
            variant="outline"
            disabled={!search.track || search.applying || search.searching}
            onClick={() => sendUiAction({ type: "open-search" })}
          >
            {t("SearchCurrentTrack")}
          </Button>
        </header>
        <Card className="search-card">
          <SearchForms key={search.query_revision} search={search} t={t} />
          <SearchResults search={search} t={t} />
          <footer className="search-footer">
            <span role="status">{search.status}</span>
            <Button
              disabled={!search.can_apply}
              onClick={() => sendUiAction({ type: "apply-lyrics" })}
            >
              {t("ApplySelectedLyrics")}
            </Button>
          </footer>
        </Card>
      </div>
    </main>
  );
}
