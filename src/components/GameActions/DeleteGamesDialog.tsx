import { useDeleteGames } from '../../api/hooks';
import buttons from '../Button/Button.module.css';
import { Dialog } from '../Dialog/Dialog';
import dialog from '../Dialog/Dialog.module.css';
import { strings } from '../../i18n';
import styles from './GameActions.module.css';

const t = strings.actions;

interface DeleteGamesDialogProps {
  ids: string[];
  titles: string[];
  onClose: () => void;
  onDeleted?: () => void;
}

export function DeleteGamesDialog({ ids, titles, onClose, onDeleted }: DeleteGamesDialogProps) {
  const remove = useDeleteGames();
  return (
    <Dialog
      title={t.deleteTitle(ids.length)}
      closeLabel={t.cancel}
      onClose={onClose}
      role="alertdialog"
    >
      <p className={dialog.muted}>{t.deleteBody}</p>
      <ul className={styles.deleteList}>
        {titles.slice(0, 8).map((title) => (
          <li key={title}>{title}</li>
        ))}
        {titles.length > 8 && <li>…</li>}
      </ul>
      {remove.isError && (
        <p role="alert" className={styles.error}>
          {t.error}
        </p>
      )}
      <div className={dialog.actions}>
        <button type="button" className={buttons.secondary} data-autofocus onClick={onClose}>
          {t.cancel}
        </button>
        <button
          type="button"
          className={dialog.danger}
          disabled={remove.isPending}
          onClick={() =>
            remove.mutate(ids, {
              onSuccess: () => {
                onDeleted?.();
                onClose();
              },
            })
          }
        >
          {t.confirmDelete}
        </button>
      </div>
    </Dialog>
  );
}
