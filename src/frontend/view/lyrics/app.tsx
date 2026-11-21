// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import { LyricPlayer } from "@applemusic-like-lyrics/react";
import { type CSSProperties, useLayoutEffect, useMemo, useRef, useSyncExternalStore } from "react";
import { clampAmllPosition, currentAmllLines, documentToAmllLines } from "./amll";
import { SlotFader } from "./fade";
import { karaokeFill, karaokeFillStyle } from "./karaoke";
import { type LyricsViewState, lyricsStore, type SlotSnapshot } from "./store";
import type { LyricsStyle } from "./types";

type LyricsCssProperties = CSSProperties & Record<`--${string}`, string>;

function cssVariables(style: LyricsStyle | null): LyricsCssProperties | undefined {
  if (!style) return undefined;
  return {
    "--font-family": style.font_family,
    "--lyric-size": `${style.lyric_font_px}px`,
    "--romanization-size": `${style.romanization_font_px}px`,
    "--translation-size": `${style.translation_font_px}px`,
    "--played-color": style.played_color,
    "--unplayed-color": style.unplayed_color,
    "--romanization-color": style.romanization_color,
    "--translation-color": style.translation_color,
  };
}

function appleMusicCssVariables(style: LyricsStyle | null): LyricsCssProperties | undefined {
  if (!style) return undefined;
  return {
    "--amll-lp-font-family": style.font_family,
    "--amll-lp-font-size": `${style.lyric_font_px}px`,
    "--amll-lp-romanization-font-size": `${style.romanization_font_px}px`,
    "--amll-lp-translation-font-size": `${style.translation_font_px}px`,
    "--amll-lp-color": style.played_color,
    "--amll-lp-romanization-color": style.romanization_color,
    "--amll-lp-translation-color": style.translation_color,
  };
}

interface LyricSlotProps {
  snapshot: SlotSnapshot | null;
  /** Width of the soft karaoke edge in pixels; `0` draws a hard edge. */
  fadePx?: number;
  setSlotRef?(element: HTMLElement | null): void;
}

function wordFadeWidth(style: LyricsStyle | null): number {
  return Math.max(0, Number(style?.word_fade_width) || 0);
}

function useSlotTransition(state: LyricsViewState) {
  const slotRefs = useRef<Array<HTMLElement | null>>([null, null]);
  const initialized = useRef(false);
  const fader = useRef<SlotFader | null>(null);
  const transitionMs = useRef(0);
  transitionMs.current = Math.max(0, Number(state.style?.transition_ms) || 0);

  useLayoutEffect(() => () => fader.current?.cancel(), []);

  useLayoutEffect(() => {
    if (state.currentKey === null) return;
    const incoming = slotRefs.current[state.activeSlot];
    const outgoing = slotRefs.current[1 - state.activeSlot];
    if (!incoming || !outgoing) return;
    fader.current ??= new SlotFader();

    incoming.classList.add("active");
    outgoing.classList.remove("active");
    if (!initialized.current) {
      fader.current.show(incoming, outgoing);
      initialized.current = true;
      return;
    }
    fader.current.crossFade(incoming, outgoing, transitionMs.current);
  }, [state.activeSlot, state.currentKey]);

  return slotRefs;
}

export function LyricSlot({ snapshot, fadePx = 0, setSlotRef }: LyricSlotProps) {
  const primaryRef = useRef<HTMLDivElement>(null);
  const content = snapshot?.content;
  const karaoke = content?.karaoke;
  const text = karaoke?.text ?? content?.text ?? "";

  useLayoutEffect(() => {
    const primary = primaryRef.current;
    const played = primary?.querySelector<HTMLElement>(".played");
    if (!primary || !played) return;
    const width = primary.clientWidth;
    const fill = karaoke ? karaokeFill(primary, karaoke) : 0;
    const { clipPath, maskImage } = karaokeFillStyle(fill, width, fadePx);
    played.style.clipPath = clipPath;
    played.style.setProperty("-webkit-mask-image", maskImage);
    played.style.setProperty("mask-image", maskImage);
  }, [karaoke, fadePx]);

  return (
    <section className="slot" ref={setSlotRef} data-lyric-key={snapshot?.key}>
      <div className={`primary${karaoke ? "" : " plain"}`} ref={primaryRef}>
        <span className="base">{text}</span>
        <span className="played">{text}</span>
      </div>
      <div className="secondary romanization">{content?.romanization ?? ""}</div>
      <div className="secondary translation">{content?.translation ?? ""}</div>
    </section>
  );
}

interface AppleMusicSlotProps {
  snapshot: SlotSnapshot | null;
  fadeWidth: number;
  documentLines: ReturnType<typeof documentToAmllLines>;
  frame: LyricsViewState["frame"];
  active: boolean;
  setSlotRef(element: HTMLElement | null): void;
}

function AppleMusicSlot({
  snapshot,
  fadeWidth,
  documentLines,
  frame,
  active,
  setSlotRef,
}: AppleMusicSlotProps) {
  const lyricLines = useMemo(
    () => currentAmllLines(documentLines, snapshot?.key ?? ""),
    [documentLines, snapshot?.key],
  );
  const currentTime = clampAmllPosition(lyricLines, frame?.position_ms ?? 0);

  return (
    <section className="slot apple-music-slot" ref={setSlotRef} data-lyric-key={snapshot?.key}>
      {lyricLines.length > 0 && frame ? (
        <LyricPlayer
          className="apple-music-player"
          style={{ textAlign: "center", whiteSpace: "nowrap" }}
          lyricLines={lyricLines}
          currentTime={currentTime}
          playing={active && frame.playing}
          isSeeking={active && frame.seeking}
          alignAnchor="center"
          alignPosition={0.5}
          enableSpring={false}
          enableBlur={false}
          enableScale={false}
          hidePassedLines={false}
          wordFadeWidth={fadeWidth}
        />
      ) : null}
    </section>
  );
}

export function LyricsViewport({ state }: { state: LyricsViewState }) {
  const slotRefs = useSlotTransition(state);
  // Rebuilt only with the style: the viewport re-renders on every frame.
  const style = useMemo(() => cssVariables(state.style), [state.style]);
  const fadePx = wordFadeWidth(state.style) * (state.style?.lyric_font_px ?? 0);

  return (
    <main id="viewport" aria-live="off" style={style}>
      <LyricSlot
        snapshot={state.slots[0]}
        fadePx={fadePx}
        setSlotRef={(element) => {
          slotRefs.current[0] = element;
        }}
      />
      <LyricSlot
        snapshot={state.slots[1]}
        fadePx={fadePx}
        setSlotRef={(element) => {
          slotRefs.current[1] = element;
        }}
      />
    </main>
  );
}

export function AppleMusicLyrics({ state }: { state: LyricsViewState }) {
  const frame = state.frame;
  const documentLines = useMemo(() => documentToAmllLines(state.document), [state.document]);
  const hasCurrentLine = useMemo(
    () => currentAmllLines(documentLines, frame?.key ?? "").length > 0,
    [documentLines, frame?.key],
  );
  const slotRefs = useSlotTransition(state);
  const style = useMemo(() => appleMusicCssVariables(state.style), [state.style]);
  const fadeWidth = wordFadeWidth(state.style);
  if (!frame || frame.position_ms === null || !hasCurrentLine) {
    return <LyricsViewport state={state} />;
  }
  return (
    <main id="viewport" className="apple-music-viewport" style={style}>
      <AppleMusicSlot
        snapshot={state.slots[0]}
        fadeWidth={fadeWidth}
        documentLines={documentLines}
        frame={frame}
        active={state.activeSlot === 0}
        setSlotRef={(element) => {
          slotRefs.current[0] = element;
        }}
      />
      <AppleMusicSlot
        snapshot={state.slots[1]}
        fadeWidth={fadeWidth}
        documentLines={documentLines}
        frame={frame}
        active={state.activeSlot === 1}
        setSlotRef={(element) => {
          slotRefs.current[1] = element;
        }}
      />
    </main>
  );
}

export function LyricsApp() {
  const state = useSyncExternalStore(
    lyricsStore.subscribe,
    lyricsStore.getSnapshot,
    lyricsStore.getSnapshot,
  );
  return state.appleMusicStyle ? (
    <AppleMusicLyrics state={state} />
  ) : (
    <LyricsViewport state={state} />
  );
}
