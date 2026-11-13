import { FolderOpen } from 'lucide-react';
import { useOpenFolder } from '../../api/hooks';
import { strings } from '../../i18n';
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
          aria-label={strings.panel.openFolderLabel(name)}
          title={strings.panel.openFolder}
          disabled={open.isPending}
          onClick={() => open.mutate(sourceId)}
        >
          <FolderOpen size={16} aria-hidden="true" />
        </button>
      )}
      {open.isError && (
        <span role="alert" className={styles.error}>
          {strings.panel.openFolderError}
        </span>
      )}
    </span>
  );
}
