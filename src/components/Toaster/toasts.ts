import { create } from 'zustand';
import { strings } from '../../i18n';

export interface ToastAction {
  label: string;
  run: () => void;
}

export interface Toast {
  id: number;
  message: string;
  /** Replaces the close button (e.g. "Annuler"); running it also dismisses. */
  action?: ToastAction;
  icon?: 'ok' | 'launch';
}

/** How long a confirmation stays on screen. */
export const TOAST_MS = 4000;
/** How long changed cards stay highlighted. */
export const FLASH_MS = 1600;
/** Delay before a launch from the grid, during which it can be cancelled. */
export const LAUNCH_DELAY_MS = 2500;

interface ShowOptions {
  durationMs?: number;
  action?: ToastAction;
  icon?: Toast['icon'];
}

interface ToastState {
  toasts: Toast[];
  /** Games just changed, highlighted for a moment in the grid. */
  flashed: Set<string>;
  show: (message: string, gameIds?: string[], options?: ShowOptions) => number;
  dismiss: (id: number) => void;
}

let next = 1;

export const useToasts = create<ToastState>()((set) => ({
  toasts: [],
  flashed: new Set(),
  show: (message, gameIds = [], options = {}) => {
    const id = next++;
    const { durationMs = TOAST_MS, action, icon } = options;
    set((s) => ({
      toasts: [...s.toasts.slice(-2), { id, message, action, icon }],
      flashed: gameIds.length ? new Set([...s.flashed, ...gameIds]) : s.flashed,
    }));
    setTimeout(() => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })), durationMs);
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
    return id;
  },
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));

/** Shortcut usable outside components (mutation callbacks…). */
export const toast = (message: string, gameIds?: string[]) =>
  useToasts.getState().show(message, gameIds);

/** Games whose delayed launch is pending (a second request is ignored). */
const pending = new Set<string>();

/**
 * Launch started from the grid: a "Lancement de … / Annuler" toast first,
 * the game only starts once the delay is over without a cancel.
 */
export function launchWithUndo(game: { id: string; title: string }, launch: () => void) {
  if (pending.has(game.id)) return;
  pending.add(game.id);
  let cancelled = false;
  useToasts.getState().show(strings.toast.launching(game.title), [], {
    durationMs: LAUNCH_DELAY_MS,
    icon: 'launch',
    action: {
      label: strings.toast.cancel,
      run: () => {
        cancelled = true;
        pending.delete(game.id);
      },
    },
  });
  setTimeout(() => {
    pending.delete(game.id);
    if (!cancelled) launch();
  }, LAUNCH_DELAY_MS);
}
