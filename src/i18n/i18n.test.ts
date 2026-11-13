import { afterEach, describe, expect, it, vi } from 'vitest';
import { en } from './en';
import { fr } from './fr';
import { detectLanguage, genreLabel, storeLanguage } from '.';

/** Every leaf path of a dictionary, functions included. */
function paths(value: unknown, prefix = ''): string[] {
  if (value && typeof value === 'object' && !Array.isArray(value)) {
    return Object.entries(value).flatMap(([k, v]) => paths(v, prefix ? `${prefix}.${k}` : k));
  }
  return [prefix];
}

describe('i18n', () => {
  afterEach(() => {
    storeLanguage('fr');
    vi.unstubAllGlobals();
  });

  it('English has every French text and no extra one', () => {
    // genreNames is a free-form map (empty in French).
    const keys = (d: object) => paths(d).filter((p) => !p.startsWith('genreNames'));
    expect(keys(en).sort()).toEqual(keys(fr).sort());
  });

  it('English texts are filled in', () => {
    const empty = paths(en).filter((p) => {
      const v = p.split('.').reduce<unknown>((o, k) => (o as Record<string, unknown>)[k], en);
      return v === '' && !p.endsWith('.hint');
    });
    expect(empty).toEqual([]);
  });

  it('keeps the stored choice, else follows the system language', () => {
    storeLanguage('en');
    expect(detectLanguage()).toBe('en');
    localStorage.removeItem('ignite.language');
    vi.stubGlobal('navigator', { language: 'fr-CH' });
    expect(detectLanguage()).toBe('fr');
    vi.stubGlobal('navigator', { language: 'de-DE' });
    expect(detectLanguage()).toBe('en');
  });

  it('shows stored genre names as they are in French', () => {
    expect(genreLabel('Coopération')).toBe('Coopération');
    expect(genreLabel('Roguelike')).toBe('Roguelike');
    expect((en.genreNames as Record<string, string>)['Coopération']).toBe('Co-op');
  });
});
