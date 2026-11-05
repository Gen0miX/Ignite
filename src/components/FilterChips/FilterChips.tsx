import { useRef } from 'react';
import { useQuery } from '@tanstack/react-query';
import { commands, isTauri, unwrap, type SourceKind } from '../../api/client';
import { useLibraryCounts } from '../../api/hooks';
import { fr } from '../../i18n/fr';
import { setToken, tokenValue } from '../../lib/searchTokens';
import styles from './FilterChips.module.css';

const t = fr.filters;

interface Option {
  value: string;
  label: string;
  count?: number;
}

function Menu({
  label,
  active,
  options,
  current,
  onPick,
}: {
  label: string;
  active: boolean;
  options: Option[];
  current: string | null;
  onPick: (value: string | null) => void;
}) {
  const ref = useRef<HTMLDetailsElement>(null);
  const pick = (v: string | null) => {
    onPick(v);
    if (ref.current) ref.current.open = false;
  };
  return (
    <details ref={ref} className={styles.menu}>
      <summary className={`${styles.chip} ${active ? styles.active : ''}`}>{label} ▾</summary>
      <div className={styles.panel} role="group" aria-label={label}>
        {options.length === 0 && <p className={styles.empty}>{t.none}</p>}
        {options.map((o) => (
          <button
            key={o.value}
            type="button"
            aria-pressed={current?.toLowerCase() === o.value.toLowerCase()}
            className={styles.option}
            onClick={() => pick(current?.toLowerCase() === o.value.toLowerCase() ? null : o.value)}
          >
            <span>{o.label}</span>
            {o.count !== undefined && <span className={styles.count}>{o.count}</span>}
          </button>
        ))}
        {current && (
          <button type="button" className={styles.clear} onClick={() => pick(null)}>
            {t.clear}
          </button>
        )}
      </div>
    </details>
  );
}

function Toggle({ label, on, onToggle }: { label: string; on: boolean; onToggle: () => void }) {
  return (
    <button
      type="button"
      aria-pressed={on}
      className={`${styles.chip} ${on ? styles.active : ''}`}
      onClick={onToggle}
    >
      {label}
    </button>
  );
}

/** Filter chips (DESIGN.md › FilterChip); they edit the search text. */
export function FilterChips({
  text,
  onChange,
}: {
  text: string;
  onChange: (text: string) => void;
}) {
  const counts = useLibraryCounts();
  const facets = useQuery({
    queryKey: ['facets'],
    queryFn: () => unwrap(commands.facets()),
    enabled: isTauri(),
  });

  const chip = (key: string, label: string, options: Option[]) => {
    const current = tokenValue(text, key);
    const shown = current
      ? (options.find((o) => o.value.toLowerCase() === current.toLowerCase())?.label ?? current)
      : null;
    return (
      <Menu
        label={shown ? `${label} : ${shown}` : label}
        active={!!current}
        options={options}
        current={current}
        onPick={(v) => onChange(setToken(text, key, v))}
      />
    );
  };
  const toggle = (key: string, label: string) => {
    const on = tokenValue(text, key) === 'oui';
    return (
      <Toggle
        label={label}
        on={on}
        onToggle={() => onChange(setToken(text, key, on ? null : 'oui'))}
      />
    );
  };

  const sources: Option[] =
    counts.data?.sources.map((s) => ({
      value: s.kind,
      label: fr.sources[s.kind as SourceKind],
      count: s.count,
    })) ?? [];
  const facet = (list?: { name: string; count: number }[]) =>
    list?.map((f) => ({ value: f.name, label: f.name, count: f.count })) ?? [];

  return (
    <div className={styles.chips} role="toolbar" aria-label={t.label}>
      {chip('source', t.source, sources)}
      {toggle('installe', t.installed)}
      {chip('genre', t.genre, facet(facets.data?.genres))}
      {chip(
        'session',
        t.session,
        t.sessions.map(([value, label]) => ({ value, label })),
      )}
      {chip('joueurs', t.players, facet(facets.data?.modes))}
      {toggle('favori', t.favorites)}
    </div>
  );
}
