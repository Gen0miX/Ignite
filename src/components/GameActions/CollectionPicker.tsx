import { useState, type FormEvent } from 'react';
import { Check, Plus } from 'lucide-react';
import {
  useAddToCollection,
  useCollectionMembership,
  useCollections,
  useSetCollectionMember,
} from '../../api/hooks';
import buttons from '../Button/Button.module.css';
import { Dialog } from '../Dialog/Dialog';
import dialog from '../Dialog/Dialog.module.css';
import { fr } from '../../i18n/fr';
import styles from './GameActions.module.css';

const t = fr.picker;

interface CollectionPickerProps {
  gameIds: string[];
  onClose: () => void;
}

/**
 * Manual collections of one game (toggle) or of a selection (add), with
 * a field to create a collection on the spot.
 */
export function CollectionPicker({ gameIds, onClose }: CollectionPickerProps) {
  const single = gameIds.length === 1 ? gameIds[0]! : null;
  const { data } = useCollections();
  const manual = data?.filter((c) => c.collection.kind === 'manual') ?? [];
  const { data: members } = useCollectionMembership(
    single ?? '',
    single ? manual.map((c) => c.collection.id) : [],
  );
  const toggle = useSetCollectionMember();
  const add = useAddToCollection();
  const [added, setAdded] = useState<Set<string>>(new Set());
  const [name, setName] = useState('');

  const create = (e: FormEvent) => {
    e.preventDefault();
    const value = name.trim();
    if (!value) return;
    add.mutate(
      { newName: value, gameIds },
      {
        onSuccess: (id) => {
          setName('');
          setAdded((s) => new Set(s).add(id));
        },
      },
    );
  };

  return (
    <Dialog
      title={single ? t.title : t.titleMany(gameIds.length)}
      closeLabel={t.close}
      onClose={onClose}
    >
      {manual.length === 0 ? (
        <p className={dialog.muted}>{t.empty}</p>
      ) : (
        <ul className={styles.pickList}>
          {manual.map((c) => {
            const id = c.collection.id;
            if (single) {
              const checked = members?.has(id) ?? false;
              return (
                <li key={id}>
                  <label className={styles.pickRow}>
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={(e) =>
                        toggle.mutate({ id, gameId: single, member: e.target.checked })
                      }
                    />
                    <span className={styles.pickName}>{c.collection.name}</span>
                    <span className={styles.pickCount}>{c.count}</span>
                  </label>
                </li>
              );
            }
            const done = added.has(id);
            return (
              <li key={id} className={styles.pickRow}>
                <span className={styles.pickName}>{c.collection.name}</span>
                <span className={styles.pickCount}>{c.count}</span>
                <button
                  type="button"
                  className={buttons.secondary}
                  disabled={done || add.isPending}
                  onClick={() =>
                    add.mutate(
                      { id, gameIds },
                      { onSuccess: () => setAdded((s) => new Set(s).add(id)) },
                    )
                  }
                >
                  {done ? <Check size={16} aria-hidden="true" /> : null}
                  {done ? t.added : t.add}
                </button>
              </li>
            );
          })}
        </ul>
      )}
      <form className={styles.pickNew} onSubmit={create}>
        <label htmlFor="picker-new" className={styles.pickLabel}>
          {t.newLabel}
        </label>
        <div className={styles.pickNewRow}>
          <input
            id="picker-new"
            className={styles.input}
            value={name}
            maxLength={60}
            placeholder={t.newPlaceholder}
            onChange={(e) => setName(e.target.value)}
          />
          <button
            type="submit"
            className={buttons.primary}
            disabled={!name.trim() || add.isPending}
          >
            <Plus size={16} aria-hidden="true" />
            {t.create}
          </button>
        </div>
      </form>
      {(add.isError || toggle.isError) && (
        <p role="alert" className={styles.error}>
          {fr.actions.error}
        </p>
      )}
      <p className={dialog.muted}>
        {t.smartNote} {t.dragHint}
      </p>
    </Dialog>
  );
}
