import type { CSSProperties } from 'react';
import { ZoomIn, ZoomOut } from 'lucide-react';
import { strings } from '../../i18n';
import { COVER_DEFAULT, COVER_MAX, COVER_MIN, COVER_STEP, usePrefs } from '../../lib/prefs';
import styles from './CoverSize.module.css';

const t = strings.library;

/** Cover size of the library grid: −/+ buttons around a slider, remembered. */
export function CoverSize() {
  const size = usePrefs((p) => p.coverSize);
  const setSize = usePrefs((p) => p.setCoverSize);
  return (
    <div className={styles.control} role="group" aria-label={t.coverSize}>
      <button
        type="button"
        className={styles.button}
        aria-label={t.coverSmaller}
        title={t.coverSmaller}
        disabled={size <= COVER_MIN}
        onClick={() => setSize(size - COVER_STEP)}
      >
        <ZoomOut size={16} aria-hidden="true" />
      </button>
      <input
        type="range"
        className={styles.slider}
        min={COVER_MIN}
        max={COVER_MAX}
        step={COVER_STEP}
        value={size}
        aria-label={t.coverSize}
        aria-valuetext={t.coverSizeValue(size)}
        title={t.coverSizeHint}
        onChange={(e) => setSize(Number(e.target.value))}
        onDoubleClick={() => setSize(COVER_DEFAULT)}
        style={
          { '--fill': `${((size - COVER_MIN) / (COVER_MAX - COVER_MIN)) * 100}%` } as CSSProperties
        }
      />
      <button
        type="button"
        className={styles.button}
        aria-label={t.coverLarger}
        title={t.coverLarger}
        disabled={size >= COVER_MAX}
        onClick={() => setSize(size + COVER_STEP)}
      >
        <ZoomIn size={16} aria-hidden="true" />
      </button>
    </div>
  );
}
