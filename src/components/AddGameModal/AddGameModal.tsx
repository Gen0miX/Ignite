import { useEffect, useRef, useState, type FormEvent, type ReactNode } from 'react';
import { useNavigate } from 'react-router-dom';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { open } from '@tauri-apps/plugin-dialog';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { FileUp, FolderSearch, Gamepad2, Library, X } from 'lucide-react';
import { commands, unwrap, type ExeInfo } from '../../api/client';
import { fr } from '../../i18n/fr';
import buttons from '../Button/Button.module.css';
import styles from './AddGameModal.module.css';
import { ScanForm } from './ScanForm';

const t = fr.addGame;
type TrackChoice = 'auto' | 'file' | 'process';

interface AddGameModalProps {
  mode: 'menu' | 'exe' | 'scan';
  onMode: (mode: 'menu' | 'exe' | 'scan') => void;
  onClose: () => void;
}

function Option({
  icon,
  title,
  body,
  onClick,
  disabled,
}: {
  icon: ReactNode;
  title: string;
  body: string;
  onClick?: () => void;
  disabled?: boolean;
}) {
  return (
    <button type="button" className={styles.option} onClick={onClick} disabled={disabled}>
      <span className={styles.optionIcon} aria-hidden="true">
        {icon}
      </span>
      <span className={styles.optionTitle}>{title}</span>
      <span className={styles.optionBody}>{body}</span>
    </button>
  );
}

function ExeForm({ onDone }: { onDone: (gameId: string) => void }) {
  const client = useQueryClient();
  const [info, setInfo] = useState<ExeInfo | null>(null);
  const [title, setTitle] = useState('');
  const [args, setArgs] = useState('');
  const [track, setTrack] = useState<TrackChoice>('auto');
  const [processName, setProcessName] = useState('');
  const [hours, setHours] = useState('');
  const [dragging, setDragging] = useState(false);

  const inspect = useMutation({
    mutationFn: (path: string) => unwrap(commands.exeInspect(path)),
    onSuccess: (data) => {
      setInfo(data);
      setTitle(data.suggestedTitle);
      setArgs(data.args ?? '');
      setTrack(data.uri ? 'process' : 'auto');
    },
  });
  const add = useMutation({
    mutationFn: () => {
      const h = Number(hours.replace(',', '.'));
      return unwrap(
        commands.addExecutable({
          title,
          exePath: info?.exePath ?? null,
          uri: info?.uri ?? null,
          args: args || null,
          track:
            track === 'process'
              ? { mode: 'process', processName }
              : track === 'file'
                ? { mode: 'file' }
                : { mode: 'auto' },
          estimateMinutes: hours && Number.isFinite(h) && h > 0 ? Math.round(h * 60) : null,
        }),
      );
    },
    onSuccess: (detail) => {
      for (const queryKey of [['library'], ['counts'], ['duplicates']]) {
        void client.invalidateQueries({ queryKey });
      }
      onDone(detail.summary.id);
    },
  });

  // Drag & drop of a file onto the window (Tauri gives real paths).
  // `mutate` is a stable function (TanStack Query), safe as an effect dependency.
  const inspectFile = inspect.mutate;
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void getCurrentWebview()
      .onDragDropEvent((e) => {
        if (e.payload.type === 'over' || e.payload.type === 'enter') setDragging(true);
        else if (e.payload.type === 'leave') setDragging(false);
        else if (e.payload.type === 'drop') {
          setDragging(false);
          const path = e.payload.paths[0];
          if (path) inspectFile(path);
        }
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [inspectFile]);

  const browse = async () => {
    const picked = await open({
      multiple: false,
      directory: false,
      filters: [{ name: t.fileFilter, extensions: ['exe', 'lnk', 'url'] }],
    });
    if (typeof picked === 'string') inspect.mutate(picked);
  };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (info && title.trim()) add.mutate();
  };

  return (
    <form className={styles.form} onSubmit={submit}>
      <div className={`${styles.drop} ${dragging ? styles.dropActive : ''}`}>
        <FileUp size={22} aria-hidden="true" />
        <div className={styles.dropText}>
          {info ? (
            <span className="mono">{info.exePath ?? info.uri}</span>
          ) : (
            <span>{t.dropHint}</span>
          )}
        </div>
        <button type="button" className={buttons.secondary} onClick={() => void browse()}>
          {t.browse}
        </button>
      </div>
      {inspect.isError && (
        <p role="alert" className={styles.error}>
          {t.inspectError}
        </p>
      )}
      {info?.probablyNotAGame && (
        <p role="alert" className={styles.warn}>
          {t.notAGameWarning}
        </p>
      )}

      {info && (
        <>
          <label className={styles.field}>
            <span>{t.title}</span>
            <input
              required
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              maxLength={200}
            />
          </label>
          {info.exePath && (
            <label className={styles.field}>
              <span>{t.args}</span>
              <input
                className="mono"
                value={args}
                placeholder="-fullscreen"
                onChange={(e) => setArgs(e.target.value)}
              />
            </label>
          )}
          <fieldset className={styles.fieldset}>
            <legend>{t.track}</legend>
            {(['auto', 'file', 'process'] as const)
              .filter((m) => info.exePath || m === 'process')
              .map((m) => (
                <label key={m} className={styles.radio}>
                  <input
                    type="radio"
                    name="track"
                    checked={track === m}
                    onChange={() => setTrack(m)}
                  />
                  <span>
                    {t.trackModes[m]}
                    <span className={styles.hint}>{t.trackHints[m]}</span>
                  </span>
                </label>
              ))}
            {track === 'process' && (
              <input
                aria-label={t.processName}
                className={`${styles.input} mono`}
                placeholder="javaw.exe"
                value={processName}
                onChange={(e) => setProcessName(e.target.value)}
              />
            )}
          </fieldset>
          <label className={styles.field}>
            <span>{t.alreadyPlayed}</span>
            <input
              type="number"
              min={0}
              step={0.5}
              inputMode="decimal"
              value={hours}
              onChange={(e) => setHours(e.target.value)}
            />
          </label>
          <p className={styles.hint}>{t.coverHint}</p>
        </>
      )}

      {add.isError && (
        <p role="alert" className={styles.error}>
          {/already in the library/i.test((add.error as Error).message)
            ? t.alreadyAdded
            : t.addError}
        </p>
      )}
      <div className={styles.actions}>
        <button
          type="submit"
          className={buttons.primary}
          disabled={!info || !title.trim() || add.isPending}
        >
          {add.isPending ? t.adding : t.add}
        </button>
      </div>
    </form>
  );
}

/** "Ajouter un jeu" dialog (Multi-Ajouter mockup). */
export function AddGameModal({ mode, onMode, onClose }: AddGameModalProps) {
  const navigate = useNavigate();
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    dialogRef.current?.querySelector<HTMLElement>('button, input')?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
      previous?.focus();
    };
  }, [onClose, mode]);

  return (
    <div className={styles.overlay} onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="add-title"
        className={styles.modal}
      >
        <header className={styles.header}>
          <h2 id="add-title">
            {mode === 'exe' ? t.exeTitle : mode === 'scan' ? t.scanTitle : t.title_}
          </h2>
          <button type="button" className={styles.close} onClick={onClose} aria-label={t.close}>
            <X size={18} aria-hidden="true" />
          </button>
        </header>
        <div className={styles.body}>
          {mode === 'menu' ? (
            <div className={styles.options}>
              <Option
                icon={<Library size={22} />}
                title={t.fromLauncher}
                body={t.fromLauncherBody}
                onClick={() => navigate('/import')}
              />
              <Option
                icon={<FolderSearch size={22} />}
                title={t.scan}
                body={t.scanBody}
                onClick={() => onMode('scan')}
              />
              <Option
                icon={<FileUp size={22} />}
                title={t.exe}
                body={t.exeBody}
                onClick={() => onMode('exe')}
              />
              <Option icon={<Gamepad2 size={22} />} title={t.emulator} body={t.soon} disabled />
            </div>
          ) : mode === 'scan' ? (
            <ScanForm
              onDone={() => {
                onClose();
                navigate('/library?sort=added');
              }}
            />
          ) : (
            <ExeForm
              onDone={(id) => {
                onClose();
                navigate(`/library?game=${encodeURIComponent(id)}`);
              }}
            />
          )}
        </div>
      </div>
    </div>
  );
}
