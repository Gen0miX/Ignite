// Smart collection editor model: a flat list of condition rows combined
// with "all" or "any", converted to/from the backend rules JSON
// (ARCHITECTURE.md § 6). Nested groups or negations (e.g. from a search)
// are kept as advanced JSON instead.

export type FieldKind = 'text' | 'set' | 'source' | 'status' | 'bool' | 'number' | 'days';

export interface FieldDef {
  field: string;
  kind: FieldKind;
  /** Display unit → stored unit factor (hours → minutes = 60). */
  scale?: number;
}

export const FIELDS: FieldDef[] = [
  { field: 'title', kind: 'text' },
  { field: 'tag', kind: 'set' },
  { field: 'genre', kind: 'set' },
  { field: 'mode', kind: 'set' },
  { field: 'source', kind: 'source' },
  { field: 'status', kind: 'status' },
  { field: 'installed', kind: 'bool' },
  { field: 'favorite', kind: 'bool' },
  { field: 'playtime_minutes', kind: 'number', scale: 60 },
  { field: 'avg_session_minutes', kind: 'number' },
  { field: 'achievement_percent', kind: 'number' },
  { field: 'hltb_main_minutes', kind: 'number', scale: 60 },
  { field: 'last_played', kind: 'days' },
  { field: 'added_at', kind: 'days' },
];

export const OPS: Record<FieldKind, string[]> = {
  text: ['contains', 'eq'],
  set: ['in', 'neq'],
  source: ['in', 'neq'],
  status: ['in', 'neq'],
  bool: ['eq'],
  number: ['gt', 'lt', 'between'],
  days: ['newer_than_days', 'older_than_days'],
};

export interface Row {
  field: string;
  op: string;
  /** As typed in the form (display units). */
  value: string;
  /** Second bound for `between`. */
  value2: string;
}

export interface EditorModel {
  match: 'all' | 'any';
  rows: Row[];
}

export function fieldDef(field: string): FieldDef {
  return FIELDS.find((f) => f.field === field) ?? FIELDS[0]!;
}

export function defaultRow(field = 'tag'): Row {
  const def = fieldDef(field);
  const value =
    def.kind === 'bool'
      ? 'true'
      : def.kind === 'source'
        ? 'steam'
        : def.kind === 'status'
          ? 'todo'
          : '';
  return { field, op: OPS[def.kind][0]!, value, value2: '' };
}

const num = (s: string) => Number(s.replace(',', '.'));

/** Rows → rules JSON; `null` when a row is incomplete. */
export function toRules(model: EditorModel): string | null {
  const conds: unknown[] = [];
  for (const r of model.rows) {
    const def = fieldDef(r.field);
    const scale = def.scale ?? 1;
    let value: unknown;
    if (def.kind === 'bool') value = r.value === 'true';
    else if (def.kind === 'number' && r.op === 'between') {
      if (
        !r.value.trim() ||
        !r.value2.trim() ||
        Number.isNaN(num(r.value)) ||
        Number.isNaN(num(r.value2))
      ) {
        return null;
      }
      value = [num(r.value) * scale, num(r.value2) * scale];
    } else if (def.kind === 'number' || def.kind === 'days') {
      if (!r.value.trim() || Number.isNaN(num(r.value))) return null;
      value = num(r.value) * scale;
    } else {
      if (!r.value.trim()) return null;
      value = def.kind === 'text' ? r.value.trim() : [r.value.trim()];
    }
    conds.push({ field: r.field, op: r.op, value });
  }
  return JSON.stringify({ [model.match]: conds });
}

type Json = { [k: string]: unknown };

/** Rules JSON → rows, or `null` when it cannot be shown as flat rows. */
export function fromRules(json: string | null | undefined): EditorModel | null {
  if (!json) return { match: 'all', rows: [] };
  let parsed: Json;
  try {
    parsed = JSON.parse(json) as Json;
  } catch {
    return null;
  }
  const match = Array.isArray(parsed.all) ? 'all' : Array.isArray(parsed.any) ? 'any' : null;
  const list = match ? (parsed[match] as Json[]) : 'field' in parsed ? [parsed] : null;
  if (!list) return null;
  const rows: Row[] = [];
  for (const c of list) {
    if (typeof c.field !== 'string' || typeof c.op !== 'string') return null;
    const def = FIELDS.find((f) => f.field === c.field);
    if (!def || !OPS[def.kind].includes(c.op)) return null;
    const scale = def.scale ?? 1;
    const v = c.value;
    const show = (x: unknown) =>
      typeof x === 'number' ? String(Math.round((x / scale) * 100) / 100) : String(x ?? '');
    if (Array.isArray(v) && c.op === 'between')
      rows.push({ field: def.field, op: c.op, value: show(v[0]), value2: show(v[1]) });
    else if (Array.isArray(v)) {
      if (v.length !== 1) return null;
      rows.push({ field: def.field, op: c.op, value: String(v[0]), value2: '' });
    } else rows.push({ field: def.field, op: c.op, value: show(v), value2: '' });
  }
  return { match: match ?? 'all', rows };
}
