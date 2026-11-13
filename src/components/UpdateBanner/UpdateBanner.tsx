import { useEffect } from 'react';
import { Download, X } from 'lucide-react';
import { strings } from '../../i18n';
import { useUpdates } from '../../lib/updates';
import buttons from '../Button/Button.module.css';
import styles from './UpdateBanner.module.css';

const t = strings.updates;
/** Wait a little after startup: the library sync comes first. */
const FIRST_CHECK_MS = 8000;

/** Offers the new version once found; checks quietly a few seconds after startup. */
export function UpdateBanner() {
  const { status, version, progress, dismissed, check, install, dismiss } = useUpdates();

  useEffect(() => {
    if (import.meta.env.DEV) return;
    const id = window.setTimeout(() => void check({ silent: true }), FIRST_CHECK_MS);
    return () => window.clearTimeout(id);
  }, [check]);

  const busy = status === 'downloading' || status === 'installing';
  if (!version || (dismissed && !busy) || !(status === 'available' || busy)) return null;

  return (
    <div className={styles.banner} role="status" aria-live="polite">
      <Download size={18} aria-hidden="true" className={styles.icon} />
      <div className={styles.text}>
        <strong>{t.available(version)}</strong>
        <span>
          {status === 'downloading'
            ? t.downloading(Math.round(progress * 100))
            : status === 'installing'
              ? t.installing
              : t.availableHint}
        </span>
      </div>
      {!busy && (
        <>
          <button type="button" className={buttons.primary} onClick={() => void install()}>
            {t.install}
          </button>
          <button
            type="button"
            className={styles.close}
            onClick={dismiss}
            aria-label={t.later}
            title={t.later}
          >
            <X size={16} aria-hidden="true" />
          </button>
        </>
      )}
      {busy && (
        <span
          className={styles.progress}
          style={{ transform: `scaleX(${progress})` }}
          aria-hidden="true"
        />
      )}
    </div>
  );
}
