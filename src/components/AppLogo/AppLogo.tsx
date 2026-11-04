import { Link } from 'react-router-dom';
import { fr } from '../../i18n/fr';
import styles from './AppLogo.module.css';

export function AppLogo() {
  return (
    <Link to="/library" className={styles.logo}>
      <span className={styles.mark} aria-hidden="true">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
          <path d="M7 4.5v15l13-7.5z" />
        </svg>
      </span>
      {fr.app.name}
    </Link>
  );
}
