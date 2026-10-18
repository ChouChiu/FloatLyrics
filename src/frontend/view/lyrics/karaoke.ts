// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import type { KaraokeContent, TimedSyllable } from "./types";

export interface TextRange {
  start: number;
  end: number;
}

export function findSyllableRanges(
  text: string,
  syllables: readonly TimedSyllable[],
): Array<TextRange | null> {
  let searchFrom = 0;
  return syllables.map((syllable) => {
    if (!syllable.text) return { start: searchFrom, end: searchFrom };
    const start = text.indexOf(syllable.text, searchFrom);
    if (start < 0) return null;
    const end = start + syllable.text.length;
    searchFrom = end;
    return { start, end };
  });
}

export function syllableProgress(syllable: TimedSyllable, positionMs: number): number {
  if (positionMs < syllable.start_ms) return 0;
  if (positionMs >= syllable.end_ms) return 1;
  return (positionMs - syllable.start_ms) / (syllable.end_ms - syllable.start_ms);
}

let measurementRange: Range | null = null;

/**
 * Range reused for every measurement.
 *
 * The fill is recomputed on every frame, and the range is never observed while
 * it is being moved, so allocating a new one per measurement is pure churn.
 */
function measurementTextRange(): Range {
  measurementRange ??= document.createRange();
  return measurementRange;
}

function xAtOffset(textNode: Text, offset: number, lineRect: DOMRect): number {
  const range = measurementTextRange();
  const boundedOffset = Math.min(offset, textNode.length);
  range.setStart(textNode, boundedOffset);
  range.setEnd(textNode, boundedOffset);
  return range.getBoundingClientRect().left - lineRect.left;
}

export function karaokeFill(primary: HTMLElement, karaoke: KaraokeContent): number {
  const base = primary.querySelector<HTMLElement>(".base");
  const lineRect = primary.getBoundingClientRect();
  const textNode = base?.firstChild;
  if (!(textNode instanceof Text) || karaoke.syllables.length === 0) return lineRect.width;

  const ranges = findSyllableRanges(karaoke.text, karaoke.syllables);
  for (const [index, syllable] of karaoke.syllables.entries()) {
    if (karaoke.position_ms < syllable.start_ms) {
      const previous = index > 0 ? ranges[index - 1] : null;
      return previous ? xAtOffset(textNode, previous.end, lineRect) : 0;
    }
    if (karaoke.position_ms < syllable.end_ms) {
      const range = ranges[index];
      if (!range) return lineRect.width * (index / karaoke.syllables.length);
      const start = xAtOffset(textNode, range.start, lineRect);
      const end = xAtOffset(textNode, range.end, lineRect);
      return start + Math.max(0, end - start) * syllableProgress(syllable, karaoke.position_ms);
    }
  }
  return lineRect.width;
}

/** Styles that reveal the played layer of a karaoke line up to its fill. */
export interface KaraokeFillStyle {
  clipPath: string;
  maskImage: string;
}

/**
 * Reveals `fill` pixels of a line `width` pixels wide.
 *
 * With `fadePx` above zero the edge of the fill is a gradient that wide,
 * centred on the fill, instead of a hard cut; it collapses at both ends of the
 * line so an unplayed line shows nothing and a finished one shows everything.
 */
export function karaokeFillStyle(fill: number, width: number, fadePx: number): KaraokeFillStyle {
  const bounded = Math.min(width, Math.max(0, fill));
  if (fadePx <= 0 || bounded <= 0 || bounded >= width) {
    return { clipPath: `inset(0 ${Math.max(0, width - bounded)}px 0 0)`, maskImage: "none" };
  }
  const half = fadePx / 2;
  const solid = (bounded - half).toFixed(1);
  const clear = (bounded + half).toFixed(1);
  return {
    clipPath: "none",
    maskImage: `linear-gradient(to right, #000 ${solid}px, transparent ${clear}px)`,
  };
}
