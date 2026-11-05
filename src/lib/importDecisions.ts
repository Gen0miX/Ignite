import type { ImportCandidate, ImportDecision } from '../api/client';

export interface Selection {
  /** Ticked candidates, by key. */
  selected: Set<string>;
  /** Duplicates the user chose NOT to merge (imported as separate games). */
  keepSeparate: Set<string>;
}

export function initialSelection(candidates: ImportCandidate[]): Selection {
  return {
    selected: new Set(candidates.filter((c) => c.selected).map((c) => c.key)),
    keepSeparate: new Set(),
  };
}

/**
 * One decision per candidate that is not already imported:
 * unticked → skip (remembered), ticked duplicate → merge unless kept
 * separate, otherwise import.
 */
export function buildDecisions(candidates: ImportCandidate[], sel: Selection): ImportDecision[] {
  return candidates
    .filter((c) => c.state.kind !== 'imported')
    .map((c) => {
      if (!sel.selected.has(c.key)) return { key: c.key, action: { type: 'skip' } };
      if (c.state.kind === 'duplicate' && !sel.keepSeparate.has(c.key)) {
        return { key: c.key, action: { type: 'merge', gameId: c.state.gameId } };
      }
      return { key: c.key, action: { type: 'import' } };
    });
}

export function summary(candidates: ImportCandidate[], sel: Selection) {
  const decisions = buildDecisions(candidates, sel);
  return {
    toImport: decisions.filter((d) => d.action.type !== 'skip').length,
    merges: decisions.filter((d) => d.action.type === 'merge').length,
    skipped: decisions.filter((d) => d.action.type === 'skip').length,
  };
}
