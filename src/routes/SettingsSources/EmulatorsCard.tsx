import { useState, type FormEvent } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { open } from '@tauri-apps/plugin-dialog';
import { commands, unwrap, type EmulatorConfig } from '../../api/client';
import buttons from '../../components/Button/Button.module.css';
import { strings } from '../../i18n';
import styles from './SettingsSourcesPage.module.css';

const t = strings.settings.emulators;

const blank = (): EmulatorConfig => ({
  id: '',
  name: '',
  exePath: '',
  argsTemplate: '"{rom}"',
  romsDir: '',
  extensions: [],
  consoleId: null,
});

function EmulatorForm({
  initial,
  onSave,
  onCancel,
  saving,
  error,
}: {
  initial: EmulatorConfig;
  onSave: (e: EmulatorConfig) => void;
  onCancel: () => void;
  saving: boolean;
  error: boolean;
}) {
  const consoles = useQuery({ queryKey: ['ra-consoles'], queryFn: () => commands.raConsoles() });
  const [e, setE] = useState(initial);
  const [exts, setExts] = useState(initial.extensions.join(', '));

  const pickExe = async () => {
    const p = await open({
      multiple: false,
      filters: [{ name: strings.settings.emulators.exeFilter, extensions: ['exe'] }],
    });
    if (typeof p === 'string') {
      const name = e.name || (p.split('\\').pop() ?? '').replace(/\.exe$/i, '');
      setE({ ...e, exePath: p, name });
    }
  };
  const pickDir = async () => {
    const p = await open({ directory: true, multiple: false });
    if (typeof p === 'string') setE({ ...e, romsDir: p });
  };
  const submit = (ev: FormEvent) => {
    ev.preventDefault();
    onSave({ ...e, extensions: exts.split(/[\s,;]+/).filter(Boolean) });
  };
  const selected = consoles.data?.find((c) => c.id === e.consoleId);

  return (
    <form className={styles.emuForm} onSubmit={submit}>
      <label className={styles.field}>
        <span className={styles.label}>{t.name}</span>
        <input
          className={styles.input}
          required
          value={e.name}
          onChange={(x) => setE({ ...e, name: x.target.value })}
        />
      </label>
      <div className={styles.field}>
        <span className={styles.label}>{t.exe}</span>
        <div className={styles.inline}>
          <input className={`${styles.input} mono`} readOnly value={e.exePath} aria-label={t.exe} />
          <button type="button" className={buttons.secondary} onClick={() => void pickExe()}>
            {t.browse}
          </button>
        </div>
      </div>
      <label className={styles.field}>
        <span className={styles.label}>{t.args}</span>
        <input
          className={`${styles.input} mono`}
          value={e.argsTemplate}
          onChange={(x) => setE({ ...e, argsTemplate: x.target.value })}
        />
        <span className={styles.muted}>{t.argsHint}</span>
      </label>
      <div className={styles.field}>
        <span className={styles.label}>{t.roms}</span>
        <div className={styles.inline}>
          <input
            className={`${styles.input} mono`}
            readOnly
            value={e.romsDir}
            aria-label={t.roms}
          />
          <button type="button" className={buttons.secondary} onClick={() => void pickDir()}>
            {t.browse}
          </button>
        </div>
      </div>
      <label className={styles.field}>
        <span className={styles.label}>{t.extensions}</span>
        <input
          className={`${styles.input} mono`}
          placeholder="sfc, smc"
          value={exts}
          onChange={(x) => setExts(x.target.value)}
        />
      </label>
      <label className={styles.field}>
        <span className={styles.label}>{t.console}</span>
        <select
          className={styles.select}
          value={e.consoleId ?? ''}
          onChange={(x) =>
            setE({ ...e, consoleId: x.target.value ? Number(x.target.value) : null })
          }
        >
          <option value="">{t.noConsole}</option>
          {consoles.data?.map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
            </option>
          ))}
        </select>
        {selected && (
          <span className={styles.muted}>{selected.autoMatch ? t.autoMatch : t.manualMatch}</span>
        )}
      </label>
      {error && (
        <p role="alert" className={styles.errorText}>
          {t.saveError}
        </p>
      )}
      <div className={styles.inline}>
        <button type="submit" className={buttons.primary} disabled={saving}>
          {t.save}
        </button>
        <button type="button" className={buttons.secondary} onClick={onCancel}>
          {t.cancel}
        </button>
      </div>
    </form>
  );
}

export function RaAccount({ keyField }: { keyField: React.ReactNode }) {
  const client = useQueryClient();
  const user = useQuery({ queryKey: ['ra-user'], queryFn: () => commands.raUsernameGet() });
  const [draft, setDraft] = useState<string | null>(null);
  const save = useMutation({
    mutationFn: (u: string | null) => unwrap(commands.raUsernameSet(u)),
    onSuccess: () => {
      setDraft(null);
      void client.invalidateQueries({ queryKey: ['ra-user'] });
    },
  });
  const value = draft ?? user.data ?? '';
  return (
    <div className={styles.field}>
      <span className={styles.label} id="ra-label">
        {t.raTitle}
      </span>
      <p className={styles.muted}>{t.raHelp}</p>
      <form
        className={styles.inline}
        onSubmit={(ev) => {
          ev.preventDefault();
          save.mutate(value.trim() || null);
        }}
      >
        <label className="visually-hidden" htmlFor="ra-user">
          {t.raUser}
        </label>
        <input
          id="ra-user"
          className={styles.input}
          placeholder={t.raUser}
          value={value}
          onChange={(x) => setDraft(x.target.value)}
        />
        <button
          type="submit"
          className={buttons.secondary}
          disabled={draft === null || save.isPending}
        >
          {t.save}
        </button>
      </form>
      {keyField}
    </div>
  );
}

/** Settings › Émulateurs: emulator list (the RetroAchievements account is in Comptes et clés). */
export function EmulatorsCard() {
  const client = useQueryClient();
  const list = useQuery({ queryKey: ['emulators'], queryFn: () => commands.emulatorsGet() });
  const [editing, setEditing] = useState<{ index: number | null; value: EmulatorConfig } | null>(
    null,
  );
  const save = useMutation({
    mutationFn: async (next: EmulatorConfig[]) => {
      const saved = await unwrap(commands.emulatorsSet(next));
      await unwrap(commands.librarySync());
      return saved;
    },
    onSuccess: () => {
      setEditing(null);
      for (const queryKey of [['emulators'], ['library'], ['counts'], ['sources']]) {
        void client.invalidateQueries({ queryKey });
      }
    },
  });
  const emulators = list.data ?? [];

  return (
    <section className={styles.card} aria-label={t.title}>
      <p className={styles.muted}>{t.help}</p>
      {emulators.length > 0 && (
        <ul className={styles.emuList}>
          {emulators.map((e, i) => (
            <li key={e.id}>
              <div>
                <strong>{e.name}</strong>
                <span className={`${styles.muted} mono`}>
                  {' '}
                  {e.romsDir} · {e.extensions.join(', ')}
                </span>
              </div>
              <div className={styles.inline}>
                <button
                  type="button"
                  className={buttons.secondary}
                  onClick={() => setEditing({ index: i, value: e })}
                >
                  {t.edit}
                </button>
                <button
                  type="button"
                  className={buttons.secondary}
                  disabled={save.isPending}
                  onClick={() => {
                    if (window.confirm(t.confirmRemove(e.name)))
                      save.mutate(emulators.filter((_, j) => j !== i));
                  }}
                >
                  {t.remove}
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
      {editing ? (
        <EmulatorForm
          initial={editing.value}
          saving={save.isPending}
          error={save.isError}
          onCancel={() => setEditing(null)}
          onSave={(e) =>
            save.mutate(
              editing.index === null
                ? [...emulators, e]
                : emulators.map((x, j) => (j === editing.index ? e : x)),
            )
          }
        />
      ) : (
        <div>
          <button
            type="button"
            className={buttons.secondary}
            onClick={() => setEditing({ index: null, value: blank() })}
          >
            {t.add}
          </button>
        </div>
      )}
    </section>
  );
}
