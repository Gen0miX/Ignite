import type { ImportCandidate } from '../api/client';
import { buildDecisions, initialSelection, summary } from './importDecisions';

const c = (key: string, state: ImportCandidate['state'], selected = true): ImportCandidate => ({
  key,
  source: 'gog',
  externalId: key,
  title: key,
  installed: false,
  installPath: null,
  sizeBytes: null,
  playtimeMinutes: null,
  achievements: null,
  state,
  note: null,
  selected,
});

const list = [
  c('gog:1', { kind: 'duplicate', gameId: 'g1', title: 'Cyberpunk 2077' }),
  c('gog:2', { kind: 'new' }),
  c('gog:3', { kind: 'new' }, false),
  c('steam:4', { kind: 'imported', gameId: 'g4' }),
];

describe('import decisions', () => {
  it('follows the default selection', () => {
    const sel = initialSelection(list);
    expect(buildDecisions(list, sel)).toEqual([
      { key: 'gog:1', action: { type: 'merge', gameId: 'g1' } },
      { key: 'gog:2', action: { type: 'import' } },
      { key: 'gog:3', action: { type: 'skip' } },
    ]);
    expect(summary(list, sel)).toEqual({ toImport: 2, merges: 1, skipped: 1 });
  });

  it('imports a duplicate separately or skips it when asked', () => {
    const sel = initialSelection(list);
    sel.keepSeparate.add('gog:1');
    expect(buildDecisions(list, sel)[0]).toEqual({ key: 'gog:1', action: { type: 'import' } });
    sel.selected.delete('gog:1');
    expect(buildDecisions(list, sel)[0]).toEqual({ key: 'gog:1', action: { type: 'skip' } });
  });
});
