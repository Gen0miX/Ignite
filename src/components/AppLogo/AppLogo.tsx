import { Link } from 'react-router-dom';
import { fr } from '../../i18n/fr';
import styles from './AppLogo.module.css';

/** Ignite mark: a flame whose core is a Play triangle. */
export function IgniteMark({ size = 36 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true">
      <rect width="64" height="64" rx="14" fill="#241d19" />
      <path
        d="M32 9c6 9 17 15.5 17 29a17 17 0 0 1-34 0c0-7.5 3.4-12 7.6-16 0 5.2 2.2 8.4 5.2 9.6C26.6 22 28.6 15.6 32 9z"
        fill="#ff7a45"
      />
      <path d="M27.5 33.5v15l12-7.5z" fill="#1a0f08" />
    </svg>
  );
}

export function AppLogo() {
  return (
    <Link to="/" className={styles.logo} aria-label={`${fr.app.name} — ${fr.nav.home}`}>
      <IgniteMark />
    </Link>
  );
}
