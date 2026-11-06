import { Eye, EyeOff, FolderPlus, Trash2, X } from 'lucide-react';
import type { BulkUpdate, GameStatus, GameSummary } from '../../api/client';
import { useBulkUpdate } from '../../api/hooks';
import { fr } from '../../i18n/fr';
import { isDeletable } from '../../lib/selection';
import buttons from '../Button/Button.module.css';
import styles from './SelectionBar.module.css';

const t = fr.selection;
const STATUSES: GameStatus[] = ['todo', 'playing', 'done', 'dropped'];

interface SelectionBarProps {
  games: GameSummary[];
  total: number;
  onSelectAll: () => void;
  onClear: () => void;
  onCollections: () => void;
  onDelete: () => void;
  /** A change was applied: confirm it, then clear the selection. */
  onDone: (message: string, ids: string[]) => void;
}

/** Actions on the ticked games, docked at the bottom of the library. */
export function SelectionBar({
  games,
  total,
  onSelectAll,
  onClear,
  onCollections,
  onDelete,
  onDone,
}: SelectionBarProps) {
  const bulk = useBulkUpdate();
  const ids = games.map((g) => g.id);
  const allHidden = games.every((g) => g.hidden);
  const deletable = games.length > 0 && games.every(isDeletable);
  // mutateAsync, not mutate callbacks: the bar may unmount before the change
  // settles (games leaving a filtered view), the confirmation must still run.
  const run = (update: BulkUpdate, message: string) =>
    bulk.mutateAsync({ ids, update }).then(
      () => onDone(message, ids),
      () => undefined,
    );
  const setStatus = (status: GameStatus) =>
    run({ status }, fr.toast.status(ids.length, fr.statusSingular[status]));

  return (
    <div role="toolbar" aria-label={t.label} className={styles.bar}>
      <span className={styles.count} role="status">
        {t.count(games.length)}
      </span>
      {games.length < total && (
        <button type="button" className={styles.link} onClick={onSelectAll}>
          {t.all}
        </button>
      )}
      <span className={styles.spacer} />
      <label className={styles.status}>
        <span className="visually-hidden">{t.status}</span>
        <select
          value=""
          disabled={bulk.isPending}
          onChange={(e) => e.target.value && setStatus(e.target.value as GameStatus)}
        >
          <option value="">{t.statusPlaceholder}</option>
          {STATUSES.map((s) => (
            <option key={s} value={s}>
              {fr.statusSingular[s]}
            </option>
          ))}
        </select>
      </label>
      <button
        type="button"
        className={buttons.secondary}
        disabled={bulk.isPending}
        onClick={() =>
          run({ hidden: !allHidden }, (allHidden ? fr.toast.unhidden : fr.toast.hidden)(ids.length))
        }
      >
        {allHidden ? <Eye size={16} aria-hidden="true" /> : <EyeOff size={16} aria-hidden="true" />}
        {allHidden ? t.unhide : t.hide}
      </button>
      <button type="button" className={buttons.secondary} onClick={onCollections}>
        <FolderPlus size={16} aria-hidden="true" />
        {t.addToCollection}
      </button>
      <button
        type="button"
        className={`${buttons.secondary} ${styles.danger}`}
        aria-disabled={!deletable}
        title={deletable ? undefined : t.deleteDisabled}
        onClick={() => deletable && onDelete()}
      >
        <Trash2 size={16} aria-hidden="true" />
        {t.delete}
      </button>
      <button type="button" className={styles.clear} onClick={onClear} aria-label={t.clear}>
        <X size={18} aria-hidden="true" />
      </button>
      {bulk.isError && (
        <p role="alert" className={styles.error}>
          {fr.actions.error}
        </p>
      )}
    </div>
  );
}
