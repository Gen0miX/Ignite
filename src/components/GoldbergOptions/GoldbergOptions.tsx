import { useState, type ReactNode } from 'react';
import { RotateCcw } from 'lucide-react';
import type { GoldbergSetting } from '../../api/client';
import { useGoldbergSettings, useSetGoldbergSetting } from '../../api/hooks';
import { fr } from '../../i18n/fr';
import { Switch } from '../Switch/Switch';
import styles from './GoldbergOptions.module.css';

const t = fr.goldbergOptions;

type OptionId = keyof typeof t.options;

type Control =
  | { type: 'bool'; invert?: boolean }
  | { type: 'text'; maxLength: number; mono?: boolean }
  | { type: 'number'; min: number; max: number; step: number }
  | { type: 'choice'; choices: Record<string, string> };

interface OptionSpec {
  id: OptionId;
  control: Control;
}

/** Same ids and values as `OPTIONS` in src-tauri/src/sources/goldberg_settings.rs. */
const GROUPS: { id: keyof typeof t.groups; open: boolean; options: OptionSpec[] }[] = [
  {
    id: 'profile',
    open: true,
    options: [
      { id: 'accountName', control: { type: 'text', maxLength: 32 } },
      { id: 'language', control: { type: 'choice', choices: t.languages } },
      { id: 'ipCountry', control: { type: 'text', maxLength: 2, mono: true } },
    ],
  },
  {
    id: 'overlay',
    open: true,
    options: [
      { id: 'overlay', control: { type: 'bool' } },
      { id: 'overlayPosition', control: { type: 'choice', choices: t.positions } },
      { id: 'overlayDuration', control: { type: 'number', min: 1, max: 30, step: 0.5 } },
      { id: 'overlayHotkey', control: { type: 'text', maxLength: 40, mono: true } },
      { id: 'overlayFps', control: { type: 'bool' } },
      { id: 'overlayNoProgress', control: { type: 'bool', invert: true } },
      { id: 'overlayNoAchievements', control: { type: 'bool', invert: true } },
    ],
  },
  {
    id: 'network',
    open: false,
    options: [
      { id: 'offline', control: { type: 'bool' } },
      { id: 'disableNetworking', control: { type: 'bool' } },
      { id: 'unlockAllDlcs', control: { type: 'bool' } },
      { id: 'steamHardware', control: { type: 'choice', choices: t.hardware } },
    ],
  },
  {
    id: 'compat',
    open: false,
    options: [
      { id: 'achievementsBypass', control: { type: 'bool' } },
      { id: 'newAppTicket', control: { type: 'bool' } },
      { id: 'steamInput', control: { type: 'bool' } },
      { id: 'controllerType', control: { type: 'choice', choices: t.controllers } },
    ],
  },
];

/** Human text of a value, for the "global: …" hint. */
export function displayValue(control: Control, value: string): string {
  switch (control.type) {
    case 'bool': {
      const on = (value === '1') !== Boolean(control.invert);
      return on ? t.on : t.off;
    }
    case 'choice':
      return control.choices[value] ?? value;
    case 'number':
      return `${new Intl.NumberFormat('fr-FR', { maximumFractionDigits: 1 }).format(Number(value))} s`;
    default:
      return value;
  }
}

/** Text/number field saved on blur or Enter, only when it changed. */
function CommitField({
  id,
  value,
  control,
  onCommit,
}: {
  id: string;
  value: string;
  control: Extract<Control, { type: 'text' | 'number' }>;
  onCommit: (v: string) => void;
}) {
  // Remounted (key) when the saved value changes, so the draft starts from it.
  const [draft, setDraft] = useState(value);
  const commit = () => {
    if (draft.trim() !== value && draft.trim() !== '') onCommit(draft.trim());
    else setDraft(value);
  };
  return (
    <input
      id={id}
      className={`${styles.input} ${control.type === 'text' && control.mono ? styles.mono : ''}`}
      type={control.type === 'number' ? 'number' : 'text'}
      inputMode={control.type === 'number' ? 'decimal' : undefined}
      min={control.type === 'number' ? control.min : undefined}
      max={control.type === 'number' ? control.max : undefined}
      step={control.type === 'number' ? control.step : undefined}
      maxLength={control.type === 'text' ? control.maxLength : undefined}
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          commit();
        }
        if (e.key === 'Escape') setDraft(value);
      }}
    />
  );
}

function OptionRow({
  spec,
  setting,
  gameLevel,
  busy,
  onSet,
}: {
  spec: OptionSpec;
  setting: GoldbergSetting;
  gameLevel: boolean;
  busy: boolean;
  onSet: (value: string | null) => void;
}) {
  const text = t.options[spec.id];
  const inputId = `goldberg-opt-${gameLevel ? 'game' : 'global'}-${spec.id}`;
  const overridden = setting.value != null;
  const value = setting.value ?? setting.inherited;
  const { control } = spec;

  let field: ReactNode;
  if (control.type === 'bool') {
    const on = (value === '1') !== Boolean(control.invert);
    field = (
      <Switch
        label={text.label}
        checked={on}
        disabled={busy}
        onChange={(next) => onSet(next !== Boolean(control.invert) ? '1' : '0')}
      />
    );
  } else if (control.type === 'choice') {
    const known = value in control.choices;
    field = (
      <select
        id={inputId}
        className={styles.select}
        value={value}
        disabled={busy}
        onChange={(e) => onSet(e.target.value)}
      >
        {!known && <option value={value}>{value}</option>}
        {Object.entries(control.choices).map(([v, label]) => (
          <option key={v} value={v}>
            {label}
          </option>
        ))}
      </select>
    );
  } else {
    field = (
      <CommitField key={value} id={inputId} value={value} control={control} onCommit={onSet} />
    );
  }

  return (
    <div className={styles.row}>
      <div className={styles.text}>
        {control.type === 'bool' ? (
          <span className={styles.label}>{text.label}</span>
        ) : (
          <label className={styles.label} htmlFor={inputId}>
            {text.label}
          </label>
        )}
        {text.hint && <span className={styles.hint}>{text.hint}</span>}
      </div>
      <div className={styles.control}>
        {overridden ? (
          <button
            type="button"
            className={styles.reset}
            onClick={() => onSet(null)}
            disabled={busy}
            aria-label={gameLevel ? t.resetGame(text.label) : t.resetGlobal(text.label)}
            title={
              gameLevel
                ? `${t.resetGame(text.label)} (${displayValue(control, setting.inherited)})`
                : `${t.resetGlobal(text.label)} (${displayValue(control, setting.inherited)})`
            }
          >
            <RotateCcw size={14} aria-hidden="true" />
          </button>
        ) : (
          <span className={styles.tag} title={gameLevel ? t.inheritedTitle : t.defaultTitle}>
            {gameLevel ? t.inherited : t.defaultTag}
          </span>
        )}
        {field}
      </div>
    </div>
  );
}

/**
 * Goldberg options: global (`gameId` omitted, Réglages › Sources) or for one
 * game (its own `steam_settings`, overriding the global values).
 */
export function GoldbergOptions({ gameId }: { gameId?: string }) {
  const level = gameId ?? null;
  const settings = useGoldbergSettings(level);
  const set = useSetGoldbergSetting(level);
  const data = settings.data;
  if (!data) return null;
  const byId = new Map(data.settings.map((s) => [s.id, s]));

  return (
    <div className={styles.options}>
      <p className={styles.intro}>{gameId ? t.gameIntro : t.globalIntro}</p>
      {GROUPS.map((group) => (
        <details key={group.id} className={styles.group} open={group.open}>
          <summary>{t.groups[group.id]}</summary>
          <div className={styles.rows}>
            {group.options.map((spec) => {
              const setting = byId.get(spec.id);
              if (!setting) return null;
              return (
                <OptionRow
                  key={spec.id}
                  spec={spec}
                  setting={setting}
                  gameLevel={gameId != null}
                  busy={set.isPending}
                  onSet={(value) => set.mutate({ id: spec.id, value })}
                />
              );
            })}
          </div>
        </details>
      ))}
      <p className={styles.folder}>
        {t.folder} : <span className="mono">{data.folder}</span>
      </p>
      {set.isError && (
        <p role="alert" className={styles.error}>
          {t.error}
        </p>
      )}
    </div>
  );
}
