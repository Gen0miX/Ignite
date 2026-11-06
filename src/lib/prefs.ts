import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { StatsRange } from '../api/client';

interface Prefs {
  /** Trailer player on game sheets (Réglages › Général). */
  showTrailers: boolean;
  setShowTrailers: (v: boolean) => void;
  /** Period picked on the Statistics page; `null` until the user picks one. */
  statsRange: StatsRange | null;
  setStatsRange: (v: StatsRange) => void;
}

const DAY = 24 * 60 * 60 * 1000;

/**
 * Period shown before the user picks one: "Depuis toujours" while Ignite's
 * own history is young (under 60 days), so the page shows the Steam / GOG
 * totals instead of an almost empty year.
 */
export function defaultStatsRange(firstTrackedDay: string | null, now = new Date()): StatsRange {
  if (!firstTrackedDay) return 'all';
  const first = new Date(`${firstTrackedDay}T12:00:00`).getTime();
  return now.getTime() - first < 60 * DAY ? 'all' : 'year';
}

/** Display preferences kept in the WebView storage (no backend involved). */
export const usePrefs = create<Prefs>()(
  persist(
    (set) => ({
      showTrailers: true,
      setShowTrailers: (showTrailers) => set({ showTrailers }),
      statsRange: null,
      setStatsRange: (statsRange) => set({ statsRange }),
    }),
    { name: 'ignite.prefs' },
  ),
);
