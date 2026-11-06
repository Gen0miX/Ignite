import { describe, expect, it } from 'vitest';
import type { HomeGame } from '../../api/client';
import { pileMeta, pileTag, tonightReason } from './homeText';

const now = new Date(2026, 10, 6, 20, 0);

const hg = (extra: Partial<HomeGame['game']> = {}, more: Partial<HomeGame> = {}): HomeGame => ({
  game: {
    id: 'g',
    title: 'Hades',
    status: 'todo',
    favorite: false,
    hidden: false,
    installed: true,
    sources: ['steam'],
    playtimeMinutes: 0,
    lastPlayed: null,
    addedAt: '2026-01-01T00:00:00Z',
    cover: null,
    header: null,
    achievements: null,
    goals: null,
    ...extra,
  },
  avgSessionMinutes: null,
  coop: false,
  hltbMinutes: null,
  hero: null,
  logo: null,
  resumeNote: null,
  ...more,
});

describe('tonightReason', () => {
  it('explains a game in progress with short sessions', () => {
    const g = hg(
      { status: 'playing', playtimeMinutes: 600, lastPlayed: '2026-11-03T18:00:00Z' },
      { avgSessionMinutes: 19 },
    );
    expect(tonightReason(g, 'halfHour', now)).toBe('En cours · sessions courtes, ≈ 19 min');
    expect(tonightReason(g, 'evening', now)).toBe('En cours · ≈ 19 min / session');
  });

  it('never launched games, co-op and forgotten ones', () => {
    expect(tonightReason(hg({}, { hltbMinutes: 1800 }), 'hour', now)).toBe(
      'Jamais lancé · ≈ 30 h pour le finir',
    );
    expect(
      tonightReason(
        hg({ playtimeMinutes: 90 }, { coop: true, avgSessionMinutes: 45 }),
        'coop',
        now,
      ),
    ).toBe('Coop · ≈ 45 min / session');
    expect(
      tonightReason(
        hg({ playtimeMinutes: 90, lastPlayed: '2025-01-10T18:00:00Z' }),
        'evening',
        now,
      ),
    ).toBe('Pas lancé depuis longtemps');
    expect(
      tonightReason(
        hg({ playtimeMinutes: 90, lastPlayed: '2026-11-02T18:00:00Z' }),
        'evening',
        now,
      ),
    ).toBe('Joué il y a 4 j');
  });
});

describe('pile labels', () => {
  it('tags why the game is in the pile', () => {
    expect(pileTag(hg({ status: 'dropped', playtimeMinutes: 300 }))).toBe('Abandonné');
    expect(pileTag(hg({ playtimeMinutes: 180 }))).toBe('3 h jouées');
    expect(pileTag(hg({}, { coop: true }))).toBe('Coop');
    expect(pileTag(hg())).toBe('Jamais lancé');
  });

  it('shows the HowLongToBeat time, else whether it is installed', () => {
    expect(pileMeta(hg({}, { hltbMinutes: 1800 }))).toBe('≈ 30 h');
    expect(pileMeta(hg({ playtimeMinutes: 480 }, { hltbMinutes: 1800 }))).toBe('≈ 22 h restantes');
    expect(pileMeta(hg())).toBe('Installé');
    expect(pileMeta(hg({ installed: false }))).toBe('');
  });
});
