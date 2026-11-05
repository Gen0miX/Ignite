import { useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { Plus, Settings } from 'lucide-react';
import type { GameStatus } from '../../api/client';
import { useCollections, useLibraryCounts, useSourcesStatus } from '../../api/hooks';
import { CollectionEditor } from '../CollectionEditor/CollectionEditor';
import { fr } from '../../i18n/fr';
import { AppLogo } from '../AppLogo/AppLogo';
import { SourceChip } from '../SourceBadge/SourceBadge';
import { StatusDot } from '../StatusDot/StatusDot';
import { NowPlaying } from '../NowPlaying/NowPlaying';
import { NavItem } from './NavItem';
import styles from './Sidebar.module.css';

const STATUSES: GameStatus[] = ['playing', 'done', 'todo', 'dropped'];

export function Sidebar() {
  const { data: counts } = useLibraryCounts();
  const { data: sources } = useSourcesStatus();
  const { data: collections } = useCollections();
  const [creating, setCreating] = useState(false);
  const navigate = useNavigate();
  const syncing = sources?.some((s) => s.syncing) ?? false;
  const n = (v: number | undefined) => String(v ?? 0);

  return (
    <aside className={styles.sidebar}>
      <AppLogo />

      <nav aria-label={fr.nav.label} className={styles.group}>
        <NavItem to="/" label={fr.nav.home} />
        <NavItem to="/library" label={fr.nav.allGames} trailing={n(counts?.all)} />
        <NavItem
          to="/library?view=installed"
          label={fr.nav.installed}
          trailing={n(counts?.installed)}
        />
        <NavItem
          to="/library?view=favorites"
          label={fr.nav.favorites}
          trailing={n(counts?.favorites)}
        />
        <NavItem to="/stats" label={fr.nav.stats} trailing="→" />
        <NavItem
          to="/settings/sources"
          label={fr.nav.settings}
          trailing={<Settings size={15} aria-hidden="true" />}
        />
      </nav>

      <section aria-labelledby="sidebar-sources">
        <h2 id="sidebar-sources" className={`section-label ${styles.sectionLabel}`}>
          {fr.sidebar.sources}
        </h2>
        {counts && counts.sources.length > 0 ? (
          <ul className={styles.group}>
            {counts.sources.map((s) => (
              <li key={s.kind}>
                <NavItem
                  to={`/library?view=source-${s.kind}`}
                  label={fr.sources[s.kind]}
                  leading={<SourceChip kind={s.kind} />}
                  trailing={String(s.count)}
                />
              </li>
            ))}
          </ul>
        ) : (
          <p className={styles.hint}>{fr.sidebar.noSources}</p>
        )}
        <Link to="/settings/sources" className={styles.secondaryLink}>
          {fr.sidebar.manageSources}
        </Link>
      </section>

      <section aria-labelledby="sidebar-collections">
        <h2 id="sidebar-collections" className={`section-label ${styles.sectionLabel}`}>
          {fr.collections.section}
        </h2>
        <ul className={styles.group}>
          {collections?.map((c) => (
            <li key={c.collection.id}>
              <NavItem
                to={`/library?view=collection-${c.collection.id}`}
                label={c.collection.name}
                trailing={String(c.count)}
                size="compact"
              />
            </li>
          ))}
        </ul>
        <button type="button" className={styles.secondaryLink} onClick={() => setCreating(true)}>
          <Plus size={14} aria-hidden="true" />
          {fr.collections.add}
        </button>
        {creating && (
          <CollectionEditor
            collection={null}
            onClose={(saved) => {
              setCreating(false);
              if (saved) navigate(`/library?view=collection-${saved.id}`);
            }}
          />
        )}
      </section>

      <section aria-labelledby="sidebar-status">
        <h2 id="sidebar-status" className={`section-label ${styles.sectionLabel}`}>
          {fr.sidebar.status}
        </h2>
        <ul className={styles.group}>
          {STATUSES.map((s) => (
            <li key={s}>
              <NavItem
                to={`/library?view=status-${s}`}
                label={fr.status[s]}
                leading={<StatusDot status={s} />}
                trailing={n(counts?.status[s])}
                size="compact"
              />
            </li>
          ))}
        </ul>
      </section>

      <div className={styles.footer}>
        <NowPlaying />
        {syncing && (
          <p className={styles.hint} role="status">
            {fr.sidebar.syncing}
          </p>
        )}
        {(counts?.hidden ?? 0) > 0 && (
          <NavItem
            to="/library?view=hidden"
            label={fr.sidebar.hiddenGames}
            trailing={String(counts?.hidden)}
            size="compact"
          />
        )}
        <Link to="/import" className={styles.secondaryLink}>
          {fr.sidebar.importGames}
        </Link>
      </div>
    </aside>
  );
}
