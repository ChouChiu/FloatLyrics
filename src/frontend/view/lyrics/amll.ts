// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import type { LyricLine, LyricWord } from "@applemusic-like-lyrics/core";
import type { LyricsDocument, PresentedLyricLine } from "./types";

const LAST_LINE_FALLBACK_MS = 5_000;

function positiveEnd(value: number | null | undefined, start: number): number | null {
  return value !== null && value !== undefined && value > start ? Math.round(value) : null;
}

export function resolvedLineEnd(
  line: PresentedLyricLine,
  next: PresentedLyricLine | undefined,
  durationMs: number | null,
): number {
  const start = Math.round(line.start_ms);
  const lastSyllableEnd = line.syllables.reduce(
    (latest, syllable) => Math.max(latest, syllable.end_ms),
    start,
  );
  return (
    positiveEnd(line.end_ms, start) ??
    positiveEnd(next?.start_ms, start) ??
    positiveEnd(lastSyllableEnd, start) ??
    positiveEnd(durationMs, start) ??
    start + LAST_LINE_FALLBACK_MS
  );
}

function lineWords(line: PresentedLyricLine, endTime: number): LyricWord[] {
  const timed = line.syllables
    .filter((syllable) => syllable.text.length > 0)
    .map((syllable) => ({
      startTime: Math.round(syllable.start_ms),
      endTime: Math.max(Math.round(syllable.start_ms), Math.round(syllable.end_ms)),
      word: syllable.text,
    }));
  if (timed.length > 0) return timed;
  return [{ startTime: Math.round(line.start_ms), endTime, word: line.text }];
}

function backgroundLine(line: PresentedLyricLine, endTime: number): LyricLine | null {
  const background = line.background.trim();
  if (!background) return null;
  const startTime = Math.round(line.start_ms);
  return {
    words: [{ startTime, endTime, word: background }],
    translatedLyric: "",
    romanLyric: "",
    startTime,
    endTime,
    isBG: true,
    isDuet: false,
  };
}

export function documentToAmllLines(document: LyricsDocument | null): LyricLine[] {
  if (!document) return [];
  const result: LyricLine[] = [];
  for (const [index, line] of document.lines.entries()) {
    const startTime = Math.round(line.start_ms);
    const endTime = resolvedLineEnd(line, document.lines[index + 1], document.duration_ms);
    result.push({
      words: lineWords(line, endTime),
      translatedLyric: line.translation,
      romanLyric: line.romanization,
      startTime,
      endTime,
      isBG: false,
      isDuet: false,
    });
    const background = backgroundLine(line, endTime);
    if (background) result.push(background);
  }
  return result;
}

export function currentAmllLines(lines: LyricLine[], frameKey: string): LyricLine[] {
  const match = /^line:(\d+)$/.exec(frameKey);
  if (!match) return [];
  const targetIndex = Number(match[1]);
  let primaryIndex = 0;
  for (const [index, line] of lines.entries()) {
    if (line.isBG) continue;
    if (primaryIndex === targetIndex) {
      const current = [line];
      for (let next = index + 1; next < lines.length; next += 1) {
        const background = lines[next];
        if (!background?.isBG) break;
        current.push(background);
      }
      return current;
    }
    primaryIndex += 1;
  }
  return [];
}
