import { create } from 'zustand';
import type { RecapMode } from '../../lib/prefs';

/** What to do with a recap that just became ready. */
export type RecapDecision = 'open' | 'notify' | 'skip';

export function recapDecision(input: {
  mode: RecapMode;
  minMinutes: number;
  activeSeconds: number;
  /** Couch mode is on screen: no modal over it. */
  couch: boolean;
  /** Another game started in the meantime: do not steal its focus. */
  otherGameRunning: boolean;
}): RecapDecision {
  if (input.mode === 'never') return 'skip';
  if (input.mode === 'longer' && input.activeSeconds < input.minMinutes * 60) return 'skip';
  if (input.couch || input.otherGameRunning) return 'notify';
  return 'open';
}

interface RecapQueue {
  /** Recaps to show, one modal at a time (first = on screen). */
  queue: string[];
  /** Announced by a notification, shown when the user is back in the app. */
  pending: string[];
  open: (sessionId: string) => void;
  defer: (sessionId: string) => void;
  /** Pending recaps join the queue (window focused, couch mode left). */
  promote: () => void;
  /** The modal on screen was closed. */
  done: (sessionId: string) => void;
}

const add = (list: string[], id: string) => (list.includes(id) ? list : [...list, id]);

export const useRecapQueue = create<RecapQueue>()((set) => ({
  queue: [],
  pending: [],
  open: (id) =>
    set((s) => ({ queue: add(s.queue, id), pending: s.pending.filter((p) => p !== id) })),
  defer: (id) => set((s) => (s.queue.includes(id) ? s : { pending: add(s.pending, id) })),
  promote: () =>
    set((s) =>
      s.pending.length === 0 ? s : { queue: s.pending.reduce(add, s.queue), pending: [] },
    ),
  done: (id) => set((s) => ({ queue: s.queue.filter((q) => q !== id) })),
}));
