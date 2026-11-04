/**
 * Library nav entries share the `/library` path and differ by the `view`
 * search param (`installed`, `favorites`, none = all games). An entry is
 * active when both the path and the view match; other params (search text,
 * filters) are ignored.
 */
export function isNavActive(current: { pathname: string; search: string }, to: string): boolean {
  const target = new URL(to, 'http://app.local');
  const pathMatches =
    current.pathname === target.pathname || current.pathname.startsWith(`${target.pathname}/`);
  if (!pathMatches) return false;
  const currentView = new URLSearchParams(current.search).get('view');
  return currentView === target.searchParams.get('view');
}
