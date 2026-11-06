import { FolderOpen } from 'lucide-react';
import { useOpenFolder } from '../../api/hooks';
import { fr } from '../../i18n/fr';
import { folderName } from '../../lib/format';
import { isTauri } from '../../api/client';
import styles from './InstallFolder.module.css';

/** Folder name (full path in the tooltip) and a button opening it in the Explorer. */
export function InstallFolder({ sourceId, path }: { sourceId: string; path: string }) {
  const open = useOpenFolder();
  const name = folderName(path);
  return (
    <span className={styles.folder}>
      <span className={`mono ${styles.name}`} title={path}>
        {name}
      </span>
      {isTauri() && (
        <button
          type="button"
          className={styles.open}
          aria-label={fr.panel.openFolderLabel(name)}
          title={fr.panel.openFolder}
          disabled={open.isPending}
          onClick={() => open.mutate(sourceId)}
        >
          <FolderOpen size={16} aria-hidden="true" />
        </button>
      )}
      {open.isError && (
        <span role="alert" className={styles.error}>
          {fr.panel.openFolderError}
        </span>
      )}
    </span>
  );
}
