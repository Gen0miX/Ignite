import { isNavActive } from './navActive';

const at = (url: string) => {
  const u = new URL(url, 'http://app.local');
  return { pathname: u.pathname, search: u.search };
};

describe('isNavActive', () => {
  it('matches the library entry only without a view', () => {
    expect(isNavActive(at('/library'), '/library')).toBe(true);
    expect(isNavActive(at('/library?q=souls'), '/library')).toBe(true);
    expect(isNavActive(at('/library?view=installed'), '/library')).toBe(false);
  });

  it('matches views regardless of other params', () => {
    expect(isNavActive(at('/library?view=installed&q=x'), '/library?view=installed')).toBe(true);
    expect(isNavActive(at('/library?view=favorites'), '/library?view=installed')).toBe(false);
  });

  it('matches nested paths but not prefixes of other words', () => {
    expect(isNavActive(at('/stats'), '/stats')).toBe(true);
    expect(isNavActive(at('/stats/year'), '/stats')).toBe(true);
    expect(isNavActive(at('/statsx'), '/stats')).toBe(false);
  });
});
