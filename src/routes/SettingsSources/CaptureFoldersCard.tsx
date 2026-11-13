import { open } from '@tauri-apps/plugin-dialog';
import { useCaptureActions, useCaptureFolders } from '../../api/hooks';
import buttons from '../../components/Button/Button.module.css';
import { fr } from '../../i18n/fr';
import styles from './SettingsSourcesPage.module.css';

const t = fr.settings.captureFolders;

/**
 * Extra capture folders (recorders, a moved Game Bar folder). Read-only:
 * Ignite lists the files and never moves or deletes them.
 */
export function CaptureFoldersCard() {
  const folders = useCaptureFolders().data ?? [];
  const actions = useCaptureActions('');

  const add = async () => {
    const path = await open({ directory: true, multiple: false, title: t.pick });
    if (typeof path === 'string') actions.addFolder.mutate({ path, forGame: null });
  };

  return (
    <section className={styles.card} aria-label={t.title}>
      <p className={styles.muted}>{t.help}</p>
      {folders.length > 0 && (
        <ul className={styles.emuList}>
          {folders.map((f) => (
            <li key={f.path}>
              <div>
                <strong>{f.gameTitle ?? t.allGames}</strong>
                <span className={`${styles.muted} mono`}> {f.path}</span>
              </div>
              <div className={styles.inline}>
                <button
                  type="button"
                  className={buttons.secondary}
                  disabled={actions.removeFolder.isPending}
                  aria-label={t.removeLabel(f.path)}
                  onClick={() => actions.removeFolder.mutate(f.path)}
                >
                  {t.remove}
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
      <div className={styles.inline}>
        <button type="button" className={buttons.secondary} onClick={() => void add()}>
          {t.add}
        </button>
      </div>
      {(actions.addFolder.isError || actions.removeFolder.isError) && (
        <p role="alert" className={styles.muted}>
          {t.error}
        </p>
      )}
    </section>
  );
}
