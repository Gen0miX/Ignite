import { Flame } from 'lucide-react';
import { strings } from '../../i18n';
import styles from './FlameRating.module.css';

/** Read-only 1–5 flames (session rating from the recap); nothing when unrated. */
export function FlameRating({ value, size = 13 }: { value: number | null; size?: number }) {
  if (!value) return null;
  const rounded = Math.round(value);
  return (
    <span className={styles.flames} role="img" aria-label={strings.recap.ratingOf(value)}>
      {[1, 2, 3, 4, 5].map((n) => (
        <Flame
          key={n}
          size={size}
          fill="currentColor"
          aria-hidden="true"
          className={n <= rounded ? styles.on : styles.off}
        />
      ))}
    </span>
  );
}
