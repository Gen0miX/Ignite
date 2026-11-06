import { CheckCircle2, Play, X } from 'lucide-react';
import { fr } from '../../i18n/fr';
import { useToasts } from './toasts';
import styles from './Toaster.module.css';

/** Short confirmations at the bottom of the window, read by screen readers. */
export function Toaster() {
  const toasts = useToasts((s) => s.toasts);
  const dismiss = useToasts((s) => s.dismiss);
  return (
    <div className={styles.region} role="status" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className={styles.toast}>
          {t.icon === 'launch' ? (
            <Play size={16} fill="currentColor" aria-hidden="true" className={styles.iconLaunch} />
          ) : (
            <CheckCircle2 size={18} aria-hidden="true" className={styles.icon} />
          )}
          <span className={styles.message}>{t.message}</span>
          {t.action ? (
            <button
              type="button"
              className={styles.action}
              onClick={() => {
                t.action?.run();
                dismiss(t.id);
              }}
            >
              {t.action.label}
            </button>
          ) : (
            <button
              type="button"
              className={styles.close}
              aria-label={fr.toast.close}
              onClick={() => dismiss(t.id)}
            >
              <X size={16} aria-hidden="true" />
            </button>
          )}
        </div>
      ))}
    </div>
  );
}
