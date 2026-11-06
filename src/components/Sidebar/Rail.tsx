import type { ReactNode } from 'react';
import { Link, useLocation } from 'react-router-dom';
import {
  BarChart3,
  Download,
  Gamepad2,
  Home,
  LibraryBig,
  Settings,
  type LucideIcon,
} from 'lucide-react';
import { fr } from '../../i18n/fr';
import { AppLogo } from '../AppLogo/AppLogo';
import { RailNowPlaying } from '../NowPlaying/NowPlaying';
import styles from './Rail.module.css';

interface RailLinkProps {
  to: string;
  label: string;
  icon: LucideIcon;
  /** Paths (prefixes) for which the entry is the current screen. */
  match: (pathname: string) => boolean;
  badge?: ReactNode;
}

function RailLink({ to, label, icon: Icon, match, badge }: RailLinkProps) {
  const { pathname } = useLocation();
  const active = match(pathname);
  return (
    <Link
      to={to}
      className={`${styles.item} ${active ? styles.active : ''}`}
      aria-current={active ? 'page' : undefined}
    >
      <Icon size={22} aria-hidden="true" />
      <span className={styles.label}>{label}</span>
      {badge}
    </Link>
  );
}

const starts = (prefix: string) => (p: string) => p === prefix || p.startsWith(`${prefix}/`);

/** App screens, always visible; the library filters live in the panel next to it. */
export function Rail() {
  return (
    <nav aria-label={fr.nav.main} className={styles.rail}>
      <AppLogo />
      <div className={styles.group}>
        <RailLink to="/" label={fr.nav.home} icon={Home} match={(p) => p === '/'} />
        <RailLink
          to="/library"
          label={fr.nav.library}
          icon={LibraryBig}
          match={(p) => starts('/library')(p) || starts('/game')(p)}
        />
        <RailLink to="/stats" label={fr.nav.stats} icon={BarChart3} match={starts('/stats')} />
        <RailLink to="/couch" label={fr.nav.couch} icon={Gamepad2} match={starts('/couch')} />
      </div>
      <div className={styles.bottom}>
        <RailNowPlaying />
        <RailLink to="/import" label={fr.nav.import} icon={Download} match={starts('/import')} />
        <RailLink
          to="/settings/sources"
          label={fr.nav.settings}
          icon={Settings}
          match={starts('/settings')}
        />
      </div>
    </nav>
  );
}
