import { useCallback, useEffect, useMemo, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { RefreshCw, Search } from 'lucide-react';
import { isTauri, type LibraryView, type SortKey } from '../../api/client';
import { useLaunchGame, useLibraryCounts, useLibraryList, useSync } from '../../api/hooks';
import { PageMain } from '../../app/AppShell';
import buttons from '../../components/Button/Button.module.css';
import { EmptyState } from '../../components/EmptyState/EmptyState';
import { GameGrid } from '../../components/GameGrid/GameGrid';
import { GamePanel } from '../../components/GamePanel/GamePanel';
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { fr } from '../../i18n/fr';
import { readParams, withParams } from '../../lib/libraryParams';
import styles from './LibraryPage.module.css';

const SORTS: SortKey[] = ['lastPlayed', 'name', 'playtime', 'added'];

function titleForView(view: LibraryView): string {
  switch (view.kind) {
    case 'installed':
      return fr.library.installedTitle;
    case 'favorites':
      return fr.library.favoritesTitle;
    case 'hidden':
      return fr.library.hiddenTitle;
    case 'status':
      return fr.status[view.value];
    case 'source':
      return fr.sources[view.value];
    default:
      return fr.library.title;
  }
}

export function LibraryPage() {
  const [params, setParams] = useSearchParams();
  const { view, text, sort, selected } = readParams(params);

  // Search input is local for instant typing, pushed to the URL after a pause.
  const [draft, setDraft] = useState(text);
  // Follow external URL changes (sidebar link, back button) without an effect.
  const [syncedText, setSyncedText] = useState(text);
  if (syncedText !== text) {
    setSyncedText(text);
    setDraft(text);
  }
  useEffect(() => {
    if (draft === text) return;
    const t = setTimeout(
      () => setParams((p) => withParams(p, { text: draft }), { replace: true }),
      200,
    );
    return () => clearTimeout(t);
  }, [draft, text, setParams]);

  const query = useMemo(() => ({ view, text: text || null, sort }), [view, text, sort]);
  const list = useLibraryList(query);
  const counts = useLibraryCounts();
  const sync = useSync();
  const launch = useLaunchGame();
  const games = list.data ?? [];

  const select = useCallback(
    (id: string | null) => setParams((p) => withParams(p, { selected: id }), { replace: true }),
    [setParams],
  );
  const closePanel = useCallback(() => select(null), [select]);

  const libraryEmpty = counts.data?.all === 0;
  const sourceCount = counts.data?.sources.length ?? 0;
  const meta = `${fr.library.count(games.length)} · ${fr.library.sourceCount(sourceCount)}`;

  let body;
  if (!isTauri()) {
    body = <EmptyState>{fr.library.backendUnavailable}</EmptyState>;
  } else if (list.isPending) {
    body = <p className={styles.muted}>{fr.library.loading}</p>;
  } else if (libraryEmpty) {
    body = (
      <EmptyState
        title={fr.library.emptyTitle}
        action={
          <div className={styles.actions}>
            <button
              type="button"
              className={buttons.primary}
              onClick={() => sync.mutate()}
              disabled={sync.isPending}
            >
              {sync.isPending ? fr.library.syncing : fr.library.sync}
            </button>
            <Link to="/settings/sources" className={buttons.secondary}>
              {fr.library.openSettings}
            </Link>
          </div>
        }
      >
        {fr.library.emptyBody}
      </EmptyState>
    );
  } else if (games.length === 0) {
    body = (
      <EmptyState title={fr.library.emptyFilteredTitle}>{fr.library.emptyFilteredBody}</EmptyState>
    );
  } else {
    body = (
      <GameGrid
        games={games}
        selectedId={selected}
        onSelect={(id) => id !== selected && select(id)}
        onLaunch={(id) => launch.mutate(id)}
      />
    );
  }

  return (
    <>
      <PageMain>
        <PageHeader
          title={titleForView(view)}
          meta={isTauri() ? meta : fr.library.count(0)}
          actions={
            isTauri() && !libraryEmpty ? (
              <button
                type="button"
                className={buttons.secondary}
                onClick={() => sync.mutate()}
                disabled={sync.isPending}
              >
                <RefreshCw
                  size={16}
                  aria-hidden="true"
                  className={sync.isPending ? styles.spin : ''}
                />
                {sync.isPending ? fr.library.syncing : fr.library.sync}
              </button>
            ) : undefined
          }
        />
        {view.kind === 'hidden' && <p className={styles.muted}>{fr.library.hiddenHint}</p>}
        <div className={styles.toolbar}>
          <label className={styles.search}>
            <Search size={16} aria-hidden="true" />
            <span className="visually-hidden">{fr.library.searchLabel}</span>
            <input
              type="search"
              value={draft}
              placeholder={fr.library.searchPlaceholder}
              onChange={(e) => setDraft(e.target.value)}
            />
          </label>
          <label className={styles.sort}>
            <span>{fr.library.sortLabel}</span>
            <select
              value={sort}
              onChange={(e) =>
                setParams((p) => withParams(p, { sort: e.target.value as SortKey }), {
                  replace: true,
                })
              }
            >
              {SORTS.map((s) => (
                <option key={s} value={s}>
                  {fr.library.sort[s]}
                </option>
              ))}
            </select>
          </label>
        </div>
        {body}
      </PageMain>
      {selected && isTauri() && <GamePanel gameId={selected} onClose={closePanel} />}
    </>
  );
}
