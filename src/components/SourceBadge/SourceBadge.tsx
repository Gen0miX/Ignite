import type { SourceKind } from '../../api/client';
import { fr } from '../../i18n/fr';
import styles from './SourceBadge.module.css';

const sourceVar = (kind: SourceKind) => `var(--source-${kind})`;

/** Square 22 px chip with a 2-letter abbreviation (sidebar). */
export function SourceChip({ kind }: { kind: SourceKind }) {
  return (
    <span className={styles.chip} style={{ background: sourceVar(kind) }} aria-hidden="true">
      {fr.sourceAbbr[kind]}
    </span>
  );
}

/** Text badge in the source colour. `overlay` = on top of a cover. */
export function SourceBadge({ kind, overlay = false }: { kind: SourceKind; overlay?: boolean }) {
  return (
    <span
      className={`${styles.badge} ${overlay ? styles.overlay : ''}`}
      style={{ color: sourceVar(kind) }}
    >
      {fr.sources[kind]}
    </span>
  );
}
