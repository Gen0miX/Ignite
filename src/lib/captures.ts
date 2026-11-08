import type { CaptureItem, CaptureSession, CaptureSource } from '../api/client';

export type CaptureFilter = 'all' | CaptureSource;

export const CAPTURE_SOURCES: CaptureSource[] = ['steam', 'gamebar', 'folder'];

export interface CaptureGroup {
  key: string;
  /** Session start, or the day of the captures. */
  date: Date;
  session: CaptureSession | null;
  items: CaptureItem[];
}

const dayKey = (d: Date) =>
  `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;

/**
 * Captures by session (a session past midnight stays one group), else by
 * local day. Keeps the order of `items` (newest first).
 */
export function groupCaptures(items: CaptureItem[]): CaptureGroup[] {
  const groups = new Map<string, CaptureGroup>();
  for (const item of items) {
    const taken = new Date(item.capture.takenAt);
    const session = item.session;
    const key = session ? `session:${session.id}` : `day:${dayKey(taken)}`;
    let group = groups.get(key);
    if (!group) {
      const date = session ? new Date(session.startedAt) : taken;
      group = { key, date, session, items: [] };
      groups.set(key, group);
    }
    group.items.push(item);
  }
  return [...groups.values()];
}

export function sourceCounts(items: CaptureItem[]): Record<CaptureFilter, number> {
  const counts: Record<CaptureFilter, number> = {
    all: items.length,
    steam: 0,
    gamebar: 0,
    folder: 0,
  };
  for (const i of items) counts[i.capture.source] += 1;
  return counts;
}

export const filterCaptures = (items: CaptureItem[], filter: CaptureFilter) =>
  filter === 'all' ? items : items.filter((i) => i.capture.source === filter);

/** Last part of a Windows path. */
export const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** "29 oct. 2025" */
export const dayFormat = new Intl.DateTimeFormat('fr-FR', {
  day: 'numeric',
  month: 'short',
  year: 'numeric',
});

/** "11:22" */
export const timeFormat = new Intl.DateTimeFormat('fr-FR', { hour: '2-digit', minute: '2-digit' });
