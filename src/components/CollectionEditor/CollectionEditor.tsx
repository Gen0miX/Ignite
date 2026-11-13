import { useEffect, useRef, useState, type FormEvent } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Plus, Trash2, X } from 'lucide-react';
import { commands, isTauri, unwrap, type Collection } from '../../api/client';
import { fr } from '../../i18n/fr';
import {
  defaultRow,
  fieldDef,
  FIELDS,
  fromRules,
  OPS,
  toRules,
  type EditorModel,
  type Row,
} from '../../lib/ruleRows';
import buttons from '../Button/Button.module.css';
import styles from './CollectionEditor.module.css';

const t = fr.collections;
const SOURCES = [
  'steam',
  'gog',
  'epic',
  'battlenet',
  'ea',
  'ubisoft',
  'xbox',
  'riot',
  'local',
  'emulator',
] as const;
const STATUSES = ['todo', 'playing', 'done', 'dropped'] as const;

interface CollectionEditorProps {
  /** Existing collection, or null to create one. */
  collection: Collection | null;
  /** Rules to start from (e.g. the current search). */
  initialRules?: string | null;
  onClose: (saved?: Collection) => void;
}

type Facet = { name: string; count: number };

/** Genres and player modes present in the library (same query as the filter chips). */
function useFacets() {
  return useQuery({
    queryKey: ['facets'],
    queryFn: () => unwrap(commands.facets()),
    enabled: isTauri(),
  });
}

function FacetOptions({ items }: { items: Facet[] }) {
  return (
    <>
      {items.map((f) => (
        <option key={f.name} value={f.name}>
          {t.facetOption(f.name, f.count)}
        </option>
      ))}
    </>
  );
}

function ValueInput({ row, onChange }: { row: Row; onChange: (r: Row) => void }) {
  const def = fieldDef(row.field);
  const label = t.valueLabel;
  const facets = useFacets().data;
  // Genre and players: pick a value that exists in the library. A genre rule
  // also matches player modes, so both lists are offered there.
  if ((row.field === 'genre' || row.field === 'mode') && facets) {
    const genres = row.field === 'genre' ? facets.genres : [];
    const modes = facets.modes;
    const known = [...genres, ...modes].some((f) => f.name === row.value);
    return (
      <select
        aria-label={label}
        value={row.value}
        onChange={(e) => onChange({ ...row, value: e.target.value })}
      >
        <option value="" disabled>
          {t.pickValue}
        </option>
        {row.value && !known && <option value={row.value}>{row.value}</option>}
        {row.field === 'genre' ? (
          <>
            <optgroup label={t.genresGroup}>
              <FacetOptions items={genres} />
            </optgroup>
            <optgroup label={t.modesGroup}>
              <FacetOptions items={modes} />
            </optgroup>
          </>
        ) : (
          <FacetOptions items={modes} />
        )}
      </select>
    );
  }
  if (def.kind === 'bool') {
    return (
      <select
        aria-label={label}
        value={row.value}
        onChange={(e) => onChange({ ...row, value: e.target.value })}
      >
        <option value="true">{t.yes}</option>
        <option value="false">{t.no}</option>
      </select>
    );
  }
  if (def.kind === 'source' || def.kind === 'status') {
    const options = def.kind === 'source' ? SOURCES : STATUSES;
    return (
      <select
        aria-label={label}
        value={row.value}
        onChange={(e) => onChange({ ...row, value: e.target.value })}
      >
        {options.map((o) => (
          <option key={o} value={o}>
            {def.kind === 'source'
              ? fr.sources[o as (typeof SOURCES)[number]]
              : fr.statusSingular[o as (typeof STATUSES)[number]]}
          </option>
        ))}
      </select>
    );
  }
  const numeric = def.kind === 'number' || def.kind === 'days';
  const unit = t.units[row.field as keyof typeof t.units];
  return (
    <span className={styles.valueGroup}>
      <input
        aria-label={label}
        inputMode={numeric ? 'decimal' : undefined}
        value={row.value}
        onChange={(e) => onChange({ ...row, value: e.target.value })}
      />
      {row.op === 'between' && (
        <>
          <span>{t.and}</span>
          <input
            aria-label={t.valueLabel2}
            inputMode="decimal"
            value={row.value2}
            onChange={(e) => onChange({ ...row, value2: e.target.value })}
          />
        </>
      )}
      {unit && <span className={styles.unit}>{unit}</span>}
    </span>
  );
}

export function CollectionEditor({ collection, initialRules, onClose }: CollectionEditorProps) {
  const client = useQueryClient();
  const dialogRef = useRef<HTMLDivElement>(null);
  const isNew = collection === null;
  const startRules = collection?.rules ?? initialRules ?? null;
  const parsed = fromRules(startRules);
  const [name, setName] = useState(collection?.name ?? '');
  const [kind, setKind] = useState<'smart' | 'manual'>(collection?.kind ?? 'smart');
  const [model, setModel] = useState<EditorModel>(parsed ?? { match: 'all', rows: [] });
  const [advanced, setAdvanced] = useState<string | null>(parsed ? null : startRules);

  const rules = kind === 'smart' ? (advanced ?? toRules(model)) : null;
  const preview = useQuery({
    queryKey: ['rules-preview', rules],
    queryFn: () => unwrap(commands.rulesPreview(rules!)),
    enabled: kind === 'smart' && !!rules,
    retry: false,
  });

  const save = useMutation({
    mutationFn: () =>
      isNew
        ? unwrap(commands.collectionCreate(name, kind, rules))
        : unwrap(commands.collectionUpdate(collection.id, name, rules)),
    onSuccess: (saved) => {
      for (const queryKey of [['collections'], ['library'], ['counts']])
        void client.invalidateQueries({ queryKey });
      onClose(saved);
    },
  });

  useEffect(() => {
    dialogRef.current?.querySelector<HTMLElement>('input')?.focus();
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onClose();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  const setRow = (i: number, row: Row) =>
    setModel({ ...model, rows: model.rows.map((r, j) => (j === i ? row : r)) });
  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (name.trim() && (kind === 'manual' || rules)) save.mutate();
  };

  return (
    <div className={styles.overlay} onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="coll-title"
        className={styles.modal}
      >
        <header className={styles.header}>
          <h2 id="coll-title">{isNew ? t.newTitle : t.editTitle}</h2>
          <button
            type="button"
            className={styles.close}
            onClick={() => onClose()}
            aria-label={t.close}
          >
            <X size={18} aria-hidden="true" />
          </button>
        </header>
        <form className={styles.body} onSubmit={submit}>
          <label className={styles.field}>
            <span>{t.name}</span>
            <input required maxLength={60} value={name} onChange={(e) => setName(e.target.value)} />
          </label>
          {isNew && (
            <fieldset className={styles.kinds}>
              <legend>{t.kind}</legend>
              {(['smart', 'manual'] as const).map((k) => (
                <label key={k}>
                  <input
                    type="radio"
                    name="kind"
                    checked={kind === k}
                    onChange={() => setKind(k)}
                  />
                  <span>
                    {t.kinds[k]}
                    <span className={styles.hint}>{t.kindHints[k]}</span>
                  </span>
                </label>
              ))}
            </fieldset>
          )}

          {kind === 'smart' && advanced !== null && (
            <label className={styles.field}>
              <span>{t.advanced}</span>
              <textarea
                className="mono"
                rows={5}
                value={advanced}
                onChange={(e) => setAdvanced(e.target.value)}
              />
              <span className={styles.hint}>{t.advancedHint}</span>
            </label>
          )}

          {kind === 'smart' && advanced === null && (
            <section className={styles.rules} aria-label={t.rules}>
              <label className={styles.match}>
                <span>{t.matchLabel}</span>
                <select
                  value={model.match}
                  onChange={(e) => setModel({ ...model, match: e.target.value as 'all' | 'any' })}
                >
                  <option value="all">{t.matchAll}</option>
                  <option value="any">{t.matchAny}</option>
                </select>
              </label>
              <ul className={styles.rows}>
                {model.rows.map((row, i) => (
                  <li key={i} className={styles.row}>
                    <select
                      aria-label={t.fieldLabel}
                      value={row.field}
                      onChange={(e) => setRow(i, defaultRow(e.target.value))}
                    >
                      {FIELDS.map((f) => (
                        <option key={f.field} value={f.field}>
                          {t.fields[f.field as keyof typeof t.fields]}
                        </option>
                      ))}
                    </select>
                    <select
                      aria-label={t.opLabel}
                      value={row.op}
                      onChange={(e) => setRow(i, { ...row, op: e.target.value })}
                    >
                      {OPS[fieldDef(row.field).kind].map((op) => (
                        <option key={op} value={op}>
                          {t.ops[op as keyof typeof t.ops]}
                        </option>
                      ))}
                    </select>
                    <ValueInput row={row} onChange={(r) => setRow(i, r)} />
                    <button
                      type="button"
                      className={styles.iconButton}
                      aria-label={t.removeRule}
                      onClick={() =>
                        setModel({ ...model, rows: model.rows.filter((_, j) => j !== i) })
                      }
                    >
                      <Trash2 size={15} aria-hidden="true" />
                    </button>
                  </li>
                ))}
              </ul>
              <button
                type="button"
                className={buttons.secondary}
                onClick={() => setModel({ ...model, rows: [...model.rows, defaultRow()] })}
              >
                <Plus size={16} aria-hidden="true" />
                {t.addRule}
              </button>
            </section>
          )}

          {kind === 'smart' && (
            <p className={styles.preview} role="status">
              {!rules
                ? t.incomplete
                : preview.isError
                  ? t.invalid
                  : preview.data !== undefined
                    ? t.previewCount(preview.data)
                    : '…'}
            </p>
          )}
          {save.isError && (
            <p role="alert" className={styles.error}>
              {t.saveError}
            </p>
          )}
          <div className={styles.actions}>
            <button type="button" className={buttons.secondary} onClick={() => onClose()}>
              {t.cancel}
            </button>
            <button
              type="submit"
              className={buttons.primary}
              disabled={
                !name.trim() || (kind === 'smart' && (!rules || preview.isError)) || save.isPending
              }
            >
              {t.save}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
