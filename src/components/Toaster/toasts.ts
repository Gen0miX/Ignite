import { create } from 'zustand';

export interface Toast {
  id: number;
  message: string;
}

/** How long a confirmation stays on screen. */
export const TOAST_MS = 4000;
/** How long changed cards stay highlighted. */
export const FLASH_MS = 1600;

interface ToastState {
  toasts: Toast[];
  /** Games just changed, highlighted for a moment in the grid. */
  flashed: Set<string>;
  show: (message: string, gameIds?: string[]) => void;
  dismiss: (id: number) => void;
}

let next = 1;

export const useToasts = create<ToastState>()((set) => ({
  toasts: [],
  flashed: new Set(),
  show: (message, gameIds = []) => {
    const id = next++;
    set((s) => ({
      toasts: [...s.toasts.slice(-2), { id, message }],
      flashed: gameIds.length ? new Set([...s.flashed, ...gameIds]) : s.flashed,
    }));
    setTimeout(() => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })), TOAST_MS);
    if (gameIds.length) {
      setTimeout(
        () =>
          set((s) => {
            const flashed = new Set(s.flashed);
            for (const g of gameIds) flashed.delete(g);
            return { flashed };
          }),
        FLASH_MS,
      );
    }
  },
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));

/** Shortcut usable outside components (mutation callbacks…). */
export const toast = (message: string, gameIds?: string[]) =>
  useToasts.getState().show(message, gameIds);
