// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import { useEffect, useMemo, useState } from "react";
import { Button, Icon, Input } from "./components/ui";
import { appendUnique, moveItem, removeItem } from "./list";
import { sendUiAction, type UiState } from "./ui-store";
import { translator } from "./ui-text";

export function FontPickerWindow({ state }: { state: UiState }) {
  const incoming = state.config?.lyrics.font_order ?? [];
  const [fonts, setFonts] = useState(incoming);
  const [query, setQuery] = useState("");
  useEffect(() => setFonts(incoming), [incoming]);
  const matches = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return needle
      ? state.availableFonts.filter((family) => family.toLowerCase().includes(needle))
      : state.availableFonts;
  }, [state.availableFonts, query]);
  const config = state.config;
  if (!config) return null;
  const t = translator(state);
  const persist = (next: string[]) => {
    if (next === fonts) return;
    setFonts(next);
    const updated = structuredClone(config);
    updated.lyrics.font_order = next;
    sendUiAction({ type: "save-config", config: updated });
  };

  return (
    <main className="font-picker-window">
      <div className="font-picker-columns">
        <section className="font-column">
          <h2>{t("AvailableFonts")}</h2>
          <Input
            type="search"
            aria-label={t("FilterFonts")}
            placeholder={t("FilterFonts")}
            value={query}
            onChange={(event) => setQuery(event.currentTarget.value)}
          />
          <div className="font-list available-fonts">
            {matches.map((family) => (
              <button
                type="button"
                className="font-row available-font-row"
                data-selected={fonts.includes(family)}
                key={family}
                style={{ fontFamily: family }}
                onClick={() => persist(appendUnique(fonts, family))}
              >
                {family}
              </button>
            ))}
          </div>
        </section>
        <section className="font-column">
          <h2>{t("FontOrder")}</h2>
          <p className="font-column-hint">{t("FontOrderDescription")}</p>
          <div className="font-list selected-fonts">
            {fonts.map((family, index) => (
              <div className="font-row selected-font-row" key={family}>
                <span style={{ fontFamily: family }}>{family}</span>
                <div className="row-actions">
                  <Button
                    variant="ghost"
                    size="icon"
                    title={t("MoveFontUp")}
                    aria-label={t("MoveFontUp")}
                    disabled={index === 0}
                    onClick={() => persist(moveItem(fonts, index, -1))}
                  >
                    <Icon name="up" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    title={t("MoveFontDown")}
                    aria-label={t("MoveFontDown")}
                    disabled={index + 1 === fonts.length}
                    onClick={() => persist(moveItem(fonts, index, 1))}
                  >
                    <Icon name="down" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    title={t("RemoveFont")}
                    aria-label={t("RemoveFont")}
                    disabled={fonts.length <= 1}
                    onClick={() => persist(removeItem(fonts, index))}
                  >
                    <Icon name="remove" />
                  </Button>
                </div>
              </div>
            ))}
          </div>
        </section>
      </div>
      <footer className="font-picker-footer">
        <Button onClick={() => sendUiAction({ type: "close-font-picker" })}>{t("Done")}</Button>
      </footer>
    </main>
  );
}
