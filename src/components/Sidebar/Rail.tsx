import type { ReactNode } from 'react';
import { Link, useLocation } from 'react-router-dom';
import {
  BarChart3,
  Download,
  Gamepad2,
  Home,
  LibraryBig,
  PanelLeftClose,
  PanelLeftOpen,
  Settings,
  type LucideIcon,
} from 'lucide-react';
import { fr } from '../../i18n/fr';
import { AppLogo } from '../AppLogo/AppLogo';
import { RailNowPlaying } from '../NowPlaying/NowPlaying';
import { useMenuPrefs } from './menuPrefs';
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
  const collapsed = useMenuPrefs((s) => s.collapsed);
  return (
    <Link
      to={to}
      className={`${styles.item} ${active ? styles.active : ''}`}
      aria-current={active ? 'page' : undefined}
      title={collapsed ? label : undefined}
    >
      <Icon size={22} aria-hidden="true" />
      <span className={collapsed ? 'visually-hidden' : styles.label}>{label}</span>
      {badge}
    </Link>
  );
}

const starts = (prefix: string) => (p: string) => p === prefix || p.startsWith(`${prefix}/`);

/** App screens, always visible; the library filters live in the panel next to it. */
export function Rail() {
  const collapsed = useMenuPrefs((s) => s.collapsed);
  const toggle = useMenuPrefs((s) => s.toggle);
  const ToggleIcon = collapsed ? PanelLeftOpen : PanelLeftClose;
  const toggleLabel = collapsed ? fr.nav.expand : fr.nav.collapse;
  return (
    <nav aria-label={fr.nav.main} className={`${styles.rail} ${collapsed ? styles.collapsed : ''}`}>
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
        <button
          type="button"
          className={styles.toggle}
          aria-label={toggleLabel}
          aria-expanded={!collapsed}
          title={toggleLabel}
          onClick={toggle}
        >
          <ToggleIcon size={18} aria-hidden="true" />
        </button>
      </div>
    </nav>
  );
}
