import { useEffect, useState, type ReactNode } from 'react';
import { Link, useLocation, useNavigate } from 'react-router-dom';
import { ChevronDown, Plus } from 'lucide-react';
import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { GameStatus } from '../../api/client';
import {
  useAddToCollection,
  useCollections,
  useLibraryCounts,
  useSourcesStatus,
} from '../../api/hooks';
import { CollectionEditor } from '../CollectionEditor/CollectionEditor';
import { strings } from '../../i18n';
import { useGameDrag } from '../../lib/gameDrag';
import { SourceChip } from '../SourceBadge/SourceBadge';
import { StatusDot } from '../StatusDot/StatusDot';
import { NowPlaying } from '../NowPlaying/NowPlaying';
import { NavItem } from './NavItem';
import { Rail } from './Rail';
import styles from './Sidebar.module.css';

const STATUSES: GameStatus[] = ['playing', 'done', 'todo', 'dropped'];

type SectionId = 'collections' | 'status' | 'sources';

/** Folded sections of the library panel, remembered between launches. */
const usePanelPrefs = create<{
  folded: Partial<Record<SectionId, boolean>>;
  toggle: (id: SectionId) => void;
}>()(
  persist(
    (set) => ({
      folded: {},
      toggle: (id) => set((s) => ({ folded: { ...s.folded, [id]: !s.folded[id] } })),
    }),
    { name: 'ignite.sidebar' },
  ),
);

function Section({
  id,
  title,
  action,
  children,
}: {
  id: SectionId;
  title: string;
  action?: ReactNode;
  children: ReactNode;
}) {
  const folded = usePanelPrefs((s) => s.folded[id] ?? false);
  const toggle = usePanelPrefs((s) => s.toggle);
  const bodyId = `panel-section-${id}`;
  return (
    <section className={styles.section}>
      <div className={styles.sectionHead}>
        <button
          type="button"
          className={styles.sectionToggle}
          aria-expanded={!folded}
          aria-controls={bodyId}
          onClick={() => toggle(id)}
        >
          <ChevronDown
            size={14}
            aria-hidden="true"
            className={folded ? styles.chevronFolded : styles.chevron}
          />
          <h2 className="section-label">{title}</h2>
        </button>
        {action}
      </div>
      {!folded && (
        <div id={bodyId} className={styles.group}>
          {children}
        </div>
      )}
    </section>
  );
}

/** Library filters (views, collections, statuses, sources), next to the rail. */
function LibraryPanel() {
  const { data: counts } = useLibraryCounts();
  const { data: sources } = useSourcesStatus();
  const { data: collections } = useCollections();
  const [creating, setCreating] = useState(false);
  const navigate = useNavigate();
  const { mutate: addTo } = useAddToCollection();
  const over = useGameDrag((s) => s.drag?.over ?? null);
  const dragging = useGameDrag((s) => s.drag !== null);
  const syncing = sources?.some((s) => s.syncing) ?? false;
  const n = (v: number | undefined) => String(v ?? 0);

  // Games dropped on a manual collection are added to it.
  useEffect(() => {
    useGameDrag.setState({
      onDrop: (collectionId, gameIds) => addTo({ id: collectionId, gameIds }),
    });
    return () => useGameDrag.setState({ onDrop: null });
  }, [addTo]);

  return (
    <aside className={styles.panel}>
      <nav aria-label={strings.nav.label} className={styles.nav}>
        <p className={styles.panelTitle}>{strings.nav.library}</p>
        <ul className={styles.group}>
          <li>
            <NavItem to="/library" label={strings.nav.allGames} trailing={n(counts?.all)} />
          </li>
          <li>
            <NavItem
              to="/library?view=installed"
              label={strings.nav.installed}
              trailing={n(counts?.installed)}
            />
          </li>
          <li>
            <NavItem
              to="/library?view=favorites"
              label={strings.nav.favorites}
              trailing={n(counts?.favorites)}
            />
          </li>
        </ul>

        <Section
          id="collections"
          title={strings.collections.section}
          action={
            <button
              type="button"
              className={styles.iconButton}
              aria-label={strings.collections.add}
              title={strings.collections.add}
              onClick={() => setCreating(true)}
            >
              <Plus size={16} aria-hidden="true" />
            </button>
          }
        >
          {collections && collections.length > 0 ? (
            <ul className={styles.group}>
              {collections.map((c) => {
                const manual = c.collection.kind === 'manual';
                return (
                  <li
                    key={c.collection.id}
                    data-drop-collection={manual ? c.collection.id : undefined}
                    className={`${dragging && manual ? styles.dropTarget : ''} ${over === c.collection.id ? styles.dropOver : ''}`}
                  >
                    <NavItem
                      to={`/library?view=collection-${c.collection.id}`}
                      label={c.collection.name}
                      trailing={String(c.count)}
                      size="compact"
                    />
                  </li>
                );
              })}
            </ul>
          ) : (
            <p className={styles.hint}>{strings.sidebar.noCollections}</p>
          )}
          {dragging && <p className={styles.hint}>{strings.sidebar.dropHint}</p>}
        </Section>

        <Section id="status" title={strings.sidebar.status}>
          <ul className={styles.group}>
            {STATUSES.map((s) => (
              <li key={s}>
                <NavItem
                  to={`/library?view=status-${s}`}
                  label={strings.status[s]}
                  leading={<StatusDot status={s} />}
                  trailing={n(counts?.status[s])}
                  size="compact"
                />
              </li>
            ))}
          </ul>
        </Section>

        <Section
          id="sources"
          title={strings.sidebar.sources}
          action={
            <Link to="/settings/sources" className={styles.sectionLink}>
              {strings.sidebar.manage}
            </Link>
          }
        >
          {counts && counts.sources.length > 0 ? (
            <ul className={styles.group}>
              {counts.sources.map((s) => (
                <li key={s.kind}>
                  <NavItem
                    to={`/library?view=source-${s.kind}`}
                    label={strings.sources[s.kind]}
                    leading={<SourceChip kind={s.kind} />}
                    trailing={String(s.count)}
                    size="compact"
                  />
                </li>
              ))}
            </ul>
          ) : (
            <p className={styles.hint}>{strings.sidebar.noSources}</p>
          )}
        </Section>
      </nav>

      <div className={styles.footer}>
        <NowPlaying />
        {syncing && (
          <p className={styles.hint} role="status">
            {strings.sidebar.syncing}
          </p>
        )}
      </div>
      {creating && (
        <CollectionEditor
          collection={null}
          onClose={(saved) => {
            setCreating(false);
            if (saved) navigate(`/library?view=collection-${saved.id}`);
          }}
        />
      )}
    </aside>
  );
}

/** Rail on every screen, plus the library panel on library screens (kept when the rail is reduced). */
export function Sidebar() {
  const { pathname } = useLocation();
  const library = pathname.startsWith('/library') || pathname.startsWith('/game');
  return (
    <>
      <Rail />
      {library && <LibraryPanel />}
    </>
  );
}
