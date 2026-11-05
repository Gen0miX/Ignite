import { useCallback, useEffect, useMemo, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { Plus, RefreshCw, Search } from 'lucide-react';
import { isTauri, type CollectionInfo, type LibraryView, type SortKey } from '../../api/client';
import {
  useCollections,
  useDeleteCollection,
  useLaunchGame,
  useLibraryCounts,
  useSearchCheck,
  useLibraryList,
  useSync,
  useTrackerActive,
} from '../../api/hooks';
import { PageMain } from '../../app/AppShell';
import buttons from '../../components/Button/Button.module.css';
import { EmptyState } from '../../components/EmptyState/EmptyState';
import { FilterChips } from '../../components/FilterChips/FilterChips';
import { GameGrid } from '../../components/GameGrid/GameGrid';
import { AddGameModal } from '../../components/AddGameModal/AddGameModal';
import { CollectionEditor } from '../../components/CollectionEditor/CollectionEditor';
import { GamePanel } from '../../components/GamePanel/GamePanel';
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { fr } from '../../i18n/fr';
import { readParams, withParams } from '../../lib/libraryParams';
import styles from './LibraryPage.module.css';

const SORTS: SortKey[] = ['lastPlayed', 'name', 'playtime', 'added'];

function titleForView(view: LibraryView, collections?: CollectionInfo[]): string {
  switch (view.kind) {
    case 'collection':
      return (
        collections?.find((c) => c.collection.id === view.value)?.collection.name ??
        fr.collections.section
      );
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
  const { view, text, sort, selected, add } = readParams(params);

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
  const active = useTrackerActive();
  const playingIds = useMemo(() => new Set(active.data?.map((a) => a.gameId)), [active.data]);
  const games = list.data ?? [];
  const collections = useCollections();
  const removeCollection = useDeleteCollection();
  const check = useSearchCheck(text);
  const [editor, setEditor] = useState<{ mode: 'edit' | 'fromSearch' } | null>(null);
  const currentCollection =
    view.kind === 'collection'
      ? collections.data?.find((c) => c.collection.id === view.value)?.collection
      : undefined;

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
        playingIds={playingIds}
        onSelect={(id) => id !== selected && select(id)}
        onLaunch={(id) => launch.mutate(id)}
      />
    );
  }

  return (
    <>
      <PageMain>
        <PageHeader
          title={titleForView(view, collections.data)}
          meta={isTauri() ? meta : fr.library.count(0)}
          actions={
            isTauri() ? (
              <>
                {!libraryEmpty && (
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
                )}
                <button
                  type="button"
                  className={buttons.primary}
                  onClick={() => setParams((p) => withParams(p, { add: 'menu' }))}
                >
                  <Plus size={16} aria-hidden="true" />
                  {fr.addGame.open}
                </button>
              </>
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
              title={fr.collections.syntaxHelp}
              onChange={(e) => setDraft(e.target.value)}
            />
          </label>
          {isTauri() && text.includes(':') && (
            <button
              type="button"
              className={buttons.secondary}
              onClick={() => setEditor({ mode: 'fromSearch' })}
            >
              {fr.collections.saveSearch}
            </button>
          )}
          {currentCollection && (
            <>
              <button
                type="button"
                className={buttons.secondary}
                onClick={() => setEditor({ mode: 'edit' })}
              >
                {fr.collections.edit}
              </button>
              <button
                type="button"
                className={buttons.secondary}
                onClick={() => {
                  if (window.confirm(fr.collections.confirmRemove(currentCollection.name))) {
                    removeCollection.mutate(currentCollection.id, {
                      onSuccess: () => setParams(new URLSearchParams(), { replace: true }),
                    });
                  }
                }}
              >
                {fr.collections.remove}
              </button>
            </>
          )}
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
        {isTauri() && (
          <FilterChips
            text={draft}
            onChange={(next) => {
              setDraft(next);
              setParams((p) => withParams(p, { text: next }), { replace: true });
            }}
          />
        )}
        {check.data && check.data.warnings.length > 0 && (
          <p className={styles.warn} role="status">
            {fr.collections.unknownFilters(check.data.warnings.join(', '))}
          </p>
        )}
        {body}
      </PageMain>
      {editor && (
        <CollectionEditor
          collection={editor.mode === 'edit' ? (currentCollection ?? null) : null}
          initialRules={editor.mode === 'fromSearch' ? check.data?.rules : null}
          onClose={(saved) => {
            setEditor(null);
            if (saved && editor.mode === 'fromSearch') {
              setParams(new URLSearchParams({ view: `collection-${saved.id}` }));
            }
          }}
        />
      )}
      {selected && isTauri() && <GamePanel gameId={selected} onClose={closePanel} />}
      {add && isTauri() && (
        <AddGameModal
          mode={add}
          onMode={(mode) => setParams((p) => withParams(p, { add: mode }), { replace: true })}
          onClose={() => setParams((p) => withParams(p, { add: null }), { replace: true })}
        />
      )}
    </>
  );
}
