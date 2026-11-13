import type { SourceKind } from '../../api/client';
import { strings } from '../../i18n';
import { SourceIcon } from './SourceIcon';
import styles from './SourceBadge.module.css';

const sourceVar = (kind: SourceKind) => `var(--source-${kind})`;

/** Square 22 px chip: the platform logo on the source colour (sidebar, settings). */
export function SourceChip({ kind }: { kind: SourceKind }) {
  return (
    <span
      className={styles.chip}
      style={{ background: sourceVar(kind) }}
      title={strings.sources[kind]}
      aria-hidden="true"
    >
      <SourceIcon kind={kind} size={18} />
    </span>
  );
}

/**
 * Logo and name in the source colour. `overlay` = on top of a cover: logo
 * only, the name stays for screen readers and in the tooltip.
 */
export function SourceBadge({ kind, overlay = false }: { kind: SourceKind; overlay?: boolean }) {
  return (
    <span
      className={`${styles.badge} ${overlay ? styles.overlay : ''}`}
      style={{ color: sourceVar(kind) }}
      title={overlay ? strings.sources[kind] : undefined}
    >
      <SourceIcon kind={kind} size={18} />
      <span className={overlay ? 'visually-hidden' : undefined}>{strings.sources[kind]}</span>
    </span>
  );
}
