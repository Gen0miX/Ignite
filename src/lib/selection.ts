/** Multi-selection in the library grid. */

/** Only games made of hand-added executables can be deleted (launcher games would come back). */
export function isDeletable(g: { sources: readonly string[] }): boolean {
  return g.sources.length > 0 && g.sources.every((k) => k === 'local');
}

/**
 * Next selection after a click: Ctrl / selection mode toggles one game,
 * Shift adds the range from the anchor (the last game clicked).
 */
export function clickSelection(
  current: ReadonlySet<string>,
  ids: readonly string[],
  id: string,
  anchor: string | null,
  shift: boolean,
): Set<string> {
  const next = new Set(current);
  if (shift && anchor) {
    const a = ids.indexOf(anchor);
    const b = ids.indexOf(id);
    if (a >= 0 && b >= 0) {
      const [from, to] = a < b ? [a, b] : [b, a];
      for (let i = from; i <= to; i++) next.add(ids[i]!);
      return next;
    }
  }
  if (next.has(id)) next.delete(id);
  else next.add(id);
  return next;
}

/** Keeps only the selected games still in the list (after a filter or a deletion). */
export function pruneSelection(current: ReadonlySet<string>, ids: readonly string[]): Set<string> {
  const visible = new Set(ids);
  return new Set([...current].filter((id) => visible.has(id)));
}
