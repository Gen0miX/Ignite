// Automatic updates from GitHub Releases (tauri-plugin-updater). The plugin
// reads `latest.json` of the latest release, checks the signature with the
// public key of tauri.conf.json, then runs the NSIS installer (passive mode).
import { create } from 'zustand';
import type { Update } from '@tauri-apps/plugin-updater';
import { isTauri } from '../api/client';

export type UpdateStatus =
  'idle' | 'checking' | 'upToDate' | 'available' | 'downloading' | 'installing' | 'error';

interface UpdateState {
  status: UpdateStatus;
  /** Version offered, when one is available. */
  version: string | null;
  /** Release notes (body of the GitHub release). */
  notes: string | null;
  /** Download progress, 0–1, while downloading. */
  progress: number;
  /** The banner was closed with « Plus tard ». */
  dismissed: boolean;
  check: (opts?: { silent?: boolean }) => Promise<void>;
  install: () => Promise<void>;
  dismiss: () => void;
}

let pending: Update | null = null;

export const useUpdates = create<UpdateState>((set, get) => ({
  status: 'idle',
  version: null,
  notes: null,
  progress: 0,
  dismissed: false,

  check: async ({ silent = false } = {}) => {
    if (!isTauri() || get().status === 'checking' || get().status === 'downloading') return;
    set({ status: 'checking' });
    try {
      const { check } = await import('@tauri-apps/plugin-updater');
      const update = await check();
      pending = update;
      if (update) {
        set({
          status: 'available',
          version: update.version,
          notes: update.body ?? null,
          dismissed: false,
        });
      } else {
        set({ status: 'upToDate', version: null, notes: null });
      }
    } catch {
      // Offline, no release yet, GitHub unavailable: never bother at startup.
      set({ status: silent ? 'idle' : 'error' });
    }
  },

  install: async () => {
    if (!pending) return;
    set({ status: 'downloading', progress: 0 });
    try {
      let total = 0;
      let done = 0;
      await pending.downloadAndInstall((event) => {
        if (event.event === 'Started') total = event.data.contentLength ?? 0;
        if (event.event === 'Progress') {
          done += event.data.chunkLength;
          if (total > 0) set({ progress: Math.min(1, done / total) });
        }
        if (event.event === 'Finished') set({ status: 'installing', progress: 1 });
      });
      const { relaunch } = await import('@tauri-apps/plugin-process');
      await relaunch();
    } catch {
      set({ status: 'error' });
    }
  },

  dismiss: () => set({ dismissed: true }),
}));
