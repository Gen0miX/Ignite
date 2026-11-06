import { create } from 'zustand';
import { persist } from 'zustand/middleware';

/** Left menu reduced to its icons (no labels, no library panel), remembered. */
export const useMenuPrefs = create<{ collapsed: boolean; toggle: () => void }>()(
  persist(
    (set) => ({
      collapsed: false,
      toggle: () => set((s) => ({ collapsed: !s.collapsed })),
    }),
    { name: 'ignite.menu' },
  ),
);
