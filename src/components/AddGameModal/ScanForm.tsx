import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { open } from '@tauri-apps/plugin-dialog';
import { FolderSearch } from 'lucide-react';
import { commands, unwrap, type FolderCandidate } from '../../api/client';
import { strings } from '../../i18n';
import buttons from '../Button/Button.module.css';
import styles from './AddGameModal.module.css';

const t = strings.addGame.scanForm;

/** "Scanner ce PC": pick a folder, review the games found, add the selection. */
export function ScanForm({ onDone }: { onDone: (added: number) => void }) {
  const client = useQueryClient();
  const [folder, setFolder] = useState<string | null>(null);
  const [rows, setRows] = useState<
    (FolderCandidate & { picked: boolean; name: string; exe: string })[]
  >([]);

  const scan = useMutation({
    mutationFn: (path: string) => unwrap(commands.foldersScan([path])),
    onSuccess: (found) =>
      setRows(found.map((c) => ({ ...c, picked: true, name: c.title, exe: c.exePath }))),
  });
  const add = useMutation({
    mutationFn: () =>
      unwrap(
        commands.addScannedGames(
          rows.filter((r) => r.picked).map((r) => ({ title: r.name, exePath: r.exe })),
        ),
      ),
    onSuccess: (n) => {
      for (const queryKey of [['library'], ['counts'], ['duplicates'], ['collections']]) {
        void client.invalidateQueries({ queryKey });
      }
      onDone(n);
    },
  });

  const browse = async () => {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === 'string') {
      setFolder(picked);
      scan.mutate(picked);
    }
  };
  const update = (i: number, patch: Partial<(typeof rows)[number]>) =>
    setRows(rows.map((r, j) => (j === i ? { ...r, ...patch } : r)));
  const count = rows.filter((r) => r.picked).length;

  return (
    <div className={styles.form}>
      <div className={styles.drop}>
        <FolderSearch size={22} aria-hidden="true" />
        <div className={styles.dropText}>
          {folder ? <span className="mono">{folder}</span> : <span>{t.hint}</span>}
        </div>
        <button type="button" className={buttons.secondary} onClick={() => void browse()}>
          {t.choose}
        </button>
      </div>
      {scan.isPending && <p className={styles.hint}>{t.scanning}</p>}
      {scan.isError && (
        <p role="alert" className={styles.error}>
          {t.error}
        </p>
      )}
      {scan.isSuccess && rows.length === 0 && <p className={styles.hint}>{t.none}</p>}
      {rows.length > 0 && (
        <ul className={styles.scanList}>
          {rows.map((r, i) => (
            <li key={r.folder}>
              <input
                type="checkbox"
                aria-label={t.pick(r.name)}
                checked={r.picked}
                onChange={(e) => update(i, { picked: e.target.checked })}
              />
              <input
                className={styles.scanTitle}
                aria-label={t.titleLabel}
                value={r.name}
                onChange={(e) => update(i, { name: e.target.value })}
              />
              <select
                className="mono"
                aria-label={t.exeLabel}
                value={r.exe}
                onChange={(e) => update(i, { exe: e.target.value })}
              >
                {[r.exePath, ...r.alternatives].map((p) => (
                  <option key={p} value={p}>
                    {p.slice(r.folder.length + 1) || p}
                  </option>
                ))}
              </select>
            </li>
          ))}
        </ul>
      )}
      {add.isError && (
        <p role="alert" className={styles.error}>
          {t.addError}
        </p>
      )}
      <div className={styles.actions}>
        <button
          type="button"
          className={buttons.primary}
          disabled={count === 0 || add.isPending}
          onClick={() => add.mutate()}
        >
          {add.isPending ? strings.addGame.adding : t.add(count)}
        </button>
      </div>
    </div>
  );
}
