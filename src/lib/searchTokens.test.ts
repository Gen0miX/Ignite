import { setToken, tokenize, tokenValue } from './searchTokens';

describe('search tokens', () => {
  it('tokenizes with quotes and negation', () => {
    expect(tokenize('zelda tag:"souls like" -source:steam heures:>20')).toEqual([
      'zelda',
      'tag:"souls like"',
      '-source:steam',
      'heures:>20',
    ]);
  });

  it('reads and replaces values', () => {
    const q = 'genre:RPG zelda';
    expect(tokenValue(q, 'genre')).toBe('RPG');
    expect(tokenValue(q, 'joueurs')).toBeNull();
    expect(setToken(q, 'genre', 'Point & click')).toBe('zelda genre:"Point & click"');
    expect(setToken(q, 'genre', null)).toBe('zelda');
    expect(setToken('', 'installe', 'oui')).toBe('installe:oui');
    expect(tokenValue('tag:"souls like"', 'tag')).toBe('souls like');
  });
});
