import type { TimeToBeat } from '../api/client';

export type TtbKey = 'main' | 'extra' | 'completionist';

export interface TtbBar {
  key: TtbKey;
  minutes: number;
  /** Width on the shared scale (the longest = 100). */
  percent: number;
}

export interface TtbChart {
  bars: TtbBar[];
  /** Position of "Toi : X h" on the same scale; capped at 100. */
  markerPercent: number;
  /** Played more than the longest bar: the marker sits at the right edge. */
  beyond: boolean;
}

/** Bars of the "Durée de vie" card, all on the scale of the longest one. */
export function ttbChart(best: TimeToBeat, playedMinutes: number): TtbChart | null {
  const values: [TtbKey, number | null][] = [
    ['main', best.mainMinutes],
    ['extra', best.extraMinutes],
    ['completionist', best.completionistMinutes],
  ];
  const known = values.filter((v): v is [TtbKey, number] => v[1] != null && v[1] > 0);
  if (known.length === 0) return null;
  const max = Math.max(...known.map(([, m]) => m));
  return {
    bars: known.map(([key, minutes]) => ({ key, minutes, percent: (minutes / max) * 100 })),
    markerPercent: Math.min(100, (playedMinutes / max) * 100),
    beyond: playedMinutes > max,
  };
}

/** What the sentence under the bars says. */
export type TtbProgress =
  | { kind: 'mainLeft'; minutes: number }
  | { kind: 'extraLeft'; minutes: number }
  | { kind: 'beyondExtra' }
  | { kind: 'beyondCompletionist' }
  | { kind: 'beyondMain' };

export function ttbProgress(best: TimeToBeat, playedMinutes: number): TtbProgress | null {
  const { mainMinutes: main, extraMinutes: extra, completionistMinutes: done } = best;
  if (main != null && playedMinutes < main)
    return { kind: 'mainLeft', minutes: main - playedMinutes };
  if (extra != null && playedMinutes < extra) {
    return { kind: 'extraLeft', minutes: extra - playedMinutes };
  }
  if (done != null && playedMinutes >= done) return { kind: 'beyondCompletionist' };
  if (extra != null) return { kind: 'beyondExtra' };
  if (main != null) return { kind: 'beyondMain' };
  return null;
}

/** "12.5" / "12,5" (hours) → minutes; empty → null; invalid → undefined. */
export function hoursToMinutes(text: string): number | null | undefined {
  const t = text.trim().replace(',', '.');
  if (!t) return null;
  const h = Number(t);
  if (!Number.isFinite(h) || h <= 0 || h > 10_000) return undefined;
  return Math.round(h * 60);
}
