import { clickSelection, isDeletable, pruneSelection } from './selection';

const ids = ['a', 'b', 'c', 'd', 'e'];

describe('selection', () => {
  it('toggles one game', () => {
    expect([...clickSelection(new Set(), ids, 'b', null, false)]).toEqual(['b']);
    expect([...clickSelection(new Set(['b']), ids, 'b', 'b', false)]).toEqual([]);
  });

  it('adds a range with shift, in both directions', () => {
    expect([...clickSelection(new Set(['b']), ids, 'd', 'b', true)].sort()).toEqual([
      'b',
      'c',
      'd',
    ]);
    expect([...clickSelection(new Set(), ids, 'a', 'c', true)].sort()).toEqual(['a', 'b', 'c']);
    // Anchor no longer listed: plain toggle.
    expect([...clickSelection(new Set(), ids, 'a', 'zz', true)]).toEqual(['a']);
  });

  it('drops games that left the list', () => {
    expect([...pruneSelection(new Set(['a', 'x']), ids)]).toEqual(['a']);
  });

  it('only deletes executable games', () => {
    expect(isDeletable({ sources: ['local'] })).toBe(true);
    expect(isDeletable({ sources: ['local', 'steam'] })).toBe(false);
    expect(isDeletable({ sources: [] })).toBe(false);
  });
});
