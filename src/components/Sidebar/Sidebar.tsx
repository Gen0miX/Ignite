import { Link } from 'react-router-dom';
import { fr } from '../../i18n/fr';
import { AppLogo } from '../AppLogo/AppLogo';
import { StatusDot, type GameStatus } from '../StatusDot/StatusDot';
import { NavItem } from './NavItem';
import styles from './Sidebar.module.css';

const STATUSES: GameStatus[] = ['playing', 'done', 'todo', 'dropped'];

export function Sidebar() {
  return (
    <aside className={styles.sidebar}>
      <AppLogo />

      <nav aria-label={fr.nav.label} className={styles.group}>
        <NavItem to="/library" label={fr.nav.allGames} trailing="0" />
        <NavItem to="/library?view=installed" label={fr.nav.installed} trailing="0" />
        <NavItem to="/library?view=favorites" label={fr.nav.favorites} trailing="0" />
        <NavItem to="/stats" label={fr.nav.stats} trailing="→" />
      </nav>

      <section aria-labelledby="sidebar-sources">
        <h2 id="sidebar-sources" className={`section-label ${styles.sectionLabel}`}>
          {fr.sidebar.sources}
        </h2>
        <p className={styles.hint}>{fr.sidebar.noSources}</p>
        <Link to="/settings/sources" className={styles.secondaryLink}>
          {fr.sidebar.manageSources}
        </Link>
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
                trailing="0"
                size="compact"
              />
            </li>
          ))}
        </ul>
      </section>

      <div className={styles.footer}>
        <Link to="/import" className={styles.secondaryLink}>
          {fr.sidebar.importGames}
        </Link>
      </div>
    </aside>
  );
}
