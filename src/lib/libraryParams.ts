// Library state lives in the URL (SPEC § 3.1: the current filter can be
// returned to): ?view=installed&q=souls&sort=playtime&game=<id>
import type { GameStatus, LibraryView, SortKey, SourceKind } from '../api/client';

const STATUSES: GameStatus[] = ['todo', 'playing', 'done', 'dropped'];
const SOURCES: SourceKind[] = [
  'steam',
  'epic',
  'gog',
  'battlenet',
  'ea',
  'ubisoft',
  'xbox',
  'riot',
  'local',
  'emulator',
];
const SORTS: SortKey[] = ['lastPlayed', 'name', 'playtime', 'added'];
export const DEFAULT_SORT: SortKey = 'lastPlayed';

export function parseView(raw: string | null): LibraryView {
  if (raw === 'installed') return { kind: 'installed' };
  if (raw === 'favorites') return { kind: 'favorites' };
  if (raw === 'hidden') return { kind: 'hidden' };
  if (raw?.startsWith('status-')) {
    const s = raw.slice(7) as GameStatus;
    if (STATUSES.includes(s)) return { kind: 'status', value: s };
  }
  if (raw?.startsWith('source-')) {
    const k = raw.slice(7) as SourceKind;
    if (SOURCES.includes(k)) return { kind: 'source', value: k };
  }
  return { kind: 'all' };
}

export function viewParam(view: LibraryView): string | null {
  switch (view.kind) {
    case 'all':
      return null;
    case 'installed':
    case 'favorites':
    case 'hidden':
      return view.kind;
    case 'status':
      return `status-${view.value}`;
    case 'source':
      return `source-${view.value}`;
  }
}

export function parseSort(raw: string | null): SortKey {
  return SORTS.includes(raw as SortKey) ? (raw as SortKey) : DEFAULT_SORT;
}

export interface LibraryParams {
  view: LibraryView;
  text: string;
  sort: SortKey;
  selected: string | null;
}

export function readParams(params: URLSearchParams): LibraryParams {
  return {
    view: parseView(params.get('view')),
    text: params.get('q') ?? '',
    sort: parseSort(params.get('sort')),
    selected: params.get('game'),
  };
}

/** Returns new params with the given changes; empty/default values are removed. */
export function withParams(
  params: URLSearchParams,
  changes: Partial<{ text: string; sort: SortKey; selected: string | null }>,
): URLSearchParams {
  const next = new URLSearchParams(params);
  const set = (key: string, value: string | null | undefined) => {
    if (value) next.set(key, value);
    else next.delete(key);
  };
  if ('text' in changes) set('q', changes.text?.trim() ? changes.text : null);
  if ('sort' in changes) set('sort', changes.sort === DEFAULT_SORT ? null : changes.sort);
  if ('selected' in changes) set('game', changes.selected);
  return next;
}
