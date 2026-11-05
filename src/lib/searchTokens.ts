// Filter chips edit the search text itself (same syntax as the search bar),
// so the URL, the parser and "save as collection" stay in sync.

/** Splits a query into tokens, keeping quoted parts together. */
export function tokenize(text: string): string[] {
  return text.match(/-?[^\s"]+:"[^"]*"|-?[^\s"]+(?:"[^"]*")?|"[^"]*"/g) ?? [];
}

const unquote = (v: string) => v.replace(/^"(.*)"$/, '$1');
const quote = (v: string) => (/\s/.test(v) ? `"${v}"` : v);
const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();

/** Value of the first `key:` token, if any. */
export function tokenValue(text: string, key: string): string | null {
  for (const t of tokenize(text)) {
    const i = t.indexOf(':');
    if (i > 0 && norm(t.slice(0, i)) === norm(key)) return unquote(t.slice(i + 1));
  }
  return null;
}

/** Sets `key:value` (replacing any previous one); `null` removes it. */
export function setToken(text: string, key: string, value: string | null): string {
  const kept = tokenize(text).filter((t) => {
    const i = t.indexOf(':');
    return !(i > 0 && norm(t.slice(0, i)) === norm(key));
  });
  if (value !== null) kept.push(`${key}:${quote(value)}`);
  return kept.join(' ');
}
