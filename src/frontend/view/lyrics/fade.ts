// SPDX-FileCopyrightText: 2026 ChouChiu
// SPDX-License-Identifier: AGPL-3.0-only

/** An element whose opacity the fader writes. */
export interface FadeTarget {
  style: { opacity: string };
}

/** Frame clock the fader steps on; `requestAnimationFrame` in the page. */
export interface FrameScheduler {
  request(callback: (now: number) => void): number;
  cancel(handle: number): void;
  now(): number;
}

export const animationFrameScheduler: FrameScheduler = {
  request: (callback) => requestAnimationFrame(callback),
  cancel: (handle) => cancelAnimationFrame(handle),
  now: () => performance.now(),
};

/** Ease-in-out progress for a linear `progress` in `[0, 1]`. */
export function easeProgress(progress: number): number {
  const clamped = Math.min(1, Math.max(0, progress));
  return clamped * clamped * (3 - 2 * clamped);
}

function opacityOf(target: FadeTarget, fallback: number): number {
  const value = Number.parseFloat(target.style.opacity);
  return Number.isFinite(value) ? value : fallback;
}

/**
 * Cross-fades two lyric slots by writing their opacity on every frame.
 *
 * WebKitGTK stops presenting new frames while a compositor-driven opacity
 * animation (`Element.animate` or a CSS transition) runs, so the overlay kept
 * showing the outgoing line until the fade ended. Opacity written from the
 * frame clock is committed with every regular frame instead.
 */
export class SlotFader {
  private handle: number | null = null;

  constructor(private readonly scheduler: FrameScheduler = animationFrameScheduler) {}

  /** Shows `incoming` and hides `outgoing` immediately. */
  show(incoming: FadeTarget, outgoing: FadeTarget): void {
    this.cancel();
    incoming.style.opacity = "1";
    outgoing.style.opacity = "0";
  }

  /**
   * Fades `incoming` in and `outgoing` out over `durationMs`, continuing from
   * the opacity each slot has now, so an interrupted fade does not jump.
   */
  crossFade(incoming: FadeTarget, outgoing: FadeTarget, durationMs: number): void {
    const fromIncoming = opacityOf(incoming, 0);
    const fromOutgoing = opacityOf(outgoing, 1);
    this.cancel();
    if (!(durationMs > 0)) {
      this.show(incoming, outgoing);
      return;
    }
    const start = this.scheduler.now();
    const step = (now: number) => {
      const progress = (now - start) / durationMs;
      const eased = easeProgress(progress);
      incoming.style.opacity = `${fromIncoming + (1 - fromIncoming) * eased}`;
      outgoing.style.opacity = `${fromOutgoing * (1 - eased)}`;
      this.handle = progress < 1 ? this.scheduler.request(step) : null;
    };
    step(start);
  }

  cancel(): void {
    if (this.handle !== null) this.scheduler.cancel(this.handle);
    this.handle = null;
  }
}
