import { parseSort, parseView, readParams, viewParam, withParams } from './libraryParams';

describe('library URL params', () => {
  it('parses views and round-trips them', () => {
    const cases = [null, 'installed', 'favorites', 'hidden', 'status-done', 'source-steam'];
    for (const raw of cases) expect(viewParam(parseView(raw))).toBe(raw);
    expect(parseView('status-unknown')).toEqual({ kind: 'all' });
    expect(parseView('source-psn')).toEqual({ kind: 'all' });
    expect(parseView('status-playing')).toEqual({ kind: 'status', value: 'playing' });
  });

  it('falls back to the default sort', () => {
    expect(parseSort('playtime')).toBe('playtime');
    expect(parseSort('bogus')).toBe('lastPlayed');
    expect(parseSort(null)).toBe('lastPlayed');
  });

  it('reads all params', () => {
    const p = readParams(new URLSearchParams('view=installed&q=souls&sort=name&game=g1'));
    expect(p).toEqual({ view: { kind: 'installed' }, text: 'souls', sort: 'name', selected: 'g1' });
  });

  it('updates params and drops defaults', () => {
    const base = new URLSearchParams('view=installed&q=x&sort=name');
    const next = withParams(base, { text: '  ', sort: 'lastPlayed', selected: 'g2' });
    expect(next.toString()).toBe('view=installed&game=g2');
    expect(withParams(next, { selected: null }).toString()).toBe('view=installed');
  });
});
