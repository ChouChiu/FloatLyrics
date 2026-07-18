// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

import { LyricPlayer } from "@applemusic-like-lyrics/react";
import { type CSSProperties, useLayoutEffect, useMemo, useRef, useSyncExternalStore } from "react";
import { currentAmllLines, documentToAmllLines } from "./amll";
import { karaokeFill } from "./karaoke";
import { type LyricsViewState, lyricsStore, type SlotSnapshot } from "./store";

type LyricsCssProperties = CSSProperties & Record<`--${string}`, string>;

function cssVariables(state: LyricsViewState): LyricsCssProperties | undefined {
  const style = state.style;
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

function appleMusicCssVariables(state: LyricsViewState): LyricsCssProperties | undefined {
  const style = state.style;
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
  setSlotRef?(element: HTMLElement | null): void;
}

function useSlotTransition(state: LyricsViewState) {
  const slotRefs = useRef<Array<HTMLElement | null>>([null, null]);
  const initialized = useRef(false);
  const transitionMs = useRef(0);
  transitionMs.current = Math.max(0, Number(state.style?.transition_ms) || 0);

  useLayoutEffect(() => {
    if (state.currentKey === null) return;
    const incoming = slotRefs.current[state.activeSlot];
    const outgoing = slotRefs.current[1 - state.activeSlot];
    if (!incoming || !outgoing) return;

    if (!initialized.current) {
      incoming.classList.add("active");
      outgoing.classList.remove("active");
      initialized.current = true;
      return;
    }

    const incomingOpacity = Number.parseFloat(getComputedStyle(incoming).opacity) || 0;
    const outgoingOpacity = Number.parseFloat(getComputedStyle(outgoing).opacity) || 0;
    for (const slot of slotRefs.current) {
      for (const animation of slot?.getAnimations() ?? []) animation.cancel();
    }
    incoming.classList.add("active");
    outgoing.classList.remove("active");
    incoming.animate([{ opacity: incomingOpacity }, { opacity: 1 }], {
      duration: transitionMs.current,
      easing: "ease",
    });
    outgoing.animate([{ opacity: outgoingOpacity }, { opacity: 0 }], {
      duration: transitionMs.current,
      easing: "ease",
    });
  }, [state.activeSlot, state.currentKey]);

  return slotRefs;
}

export function LyricSlot({ snapshot, setSlotRef }: LyricSlotProps) {
  const primaryRef = useRef<HTMLDivElement>(null);
  const content = snapshot?.content;
  const karaoke = content?.karaoke;
  const text = karaoke?.text ?? content?.text ?? "";

  useLayoutEffect(() => {
    const primary = primaryRef.current;
    const played = primary?.querySelector<HTMLElement>(".played");
    if (!primary || !played) return;
    if (!karaoke) {
      played.style.clipPath = "inset(0 100% 0 0)";
      return;
    }
    const fill = Math.min(primary.clientWidth, Math.max(0, karaokeFill(primary, karaoke)));
    played.style.clipPath = `inset(0 ${Math.max(0, primary.clientWidth - fill)}px 0 0)`;
  }, [karaoke]);

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

export function LyricsViewport({ state }: { state: LyricsViewState }) {
  const slotRefs = useSlotTransition(state);

  return (
    <main id="viewport" aria-live="off" style={cssVariables(state)}>
      <LyricSlot
        snapshot={state.slots[0]}
        setSlotRef={(element) => {
          slotRefs.current[0] = element;
        }}
      />
      <LyricSlot
        snapshot={state.slots[1]}
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
  const style = appleMusicCssVariables(state);
  if (!frame || frame.position_ms === null || !hasCurrentLine) {
    return <LyricsViewport state={state} />;
  }
  return (
    <main id="viewport" className="apple-music-viewport" style={style}>
      <LyricPlayer
        className="apple-music-player"
        style={{ textAlign: "center", whiteSpace: "nowrap" }}
        lyricLines={documentLines}
        currentTime={Math.round(frame.position_ms)}
        playing={frame.playing}
        isSeeking={frame.seeking}
        alignAnchor="center"
        alignPosition={0.5}
        wordFadeWidth={0.5}
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
