import { create } from 'zustand';
import { persist } from 'zustand/middleware';

interface Prefs {
  /** Trailer player on game sheets (Réglages › Général). */
  showTrailers: boolean;
  setShowTrailers: (v: boolean) => void;
}

/** Display preferences kept in the WebView storage (no backend involved). */
export const usePrefs = create<Prefs>()(
  persist(
    (set) => ({
      showTrailers: true,
      setShowTrailers: (showTrailers) => set({ showTrailers }),
    }),
    { name: 'ignite.prefs' },
  ),
);
