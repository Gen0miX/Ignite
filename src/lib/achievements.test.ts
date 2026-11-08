import { describe, expect, it } from 'vitest';
import type { AchievementItem } from '../api/client';
import {
  defaultView,
  filterAchievements,
  isMasked,
  monthlyUnlocks,
  rarest,
  rarity,
  rarityCounts,
  sortAchievements,
} from './achievements';

const a = (
  apiName: string,
  percent: number | null,
  unlockedAt: string | null = null,
  extra: Partial<AchievementItem> = {},
): AchievementItem => ({
  apiName,
  name: apiName,
  description: `Description de ${apiName}`,
  unlockedAt,
  globalPercent: percent,
  icon: null,
  hidden: false,
  ...extra,
});

const list = [
  a('Élan', 62),
  a('Brume', 1.8, '2019-03-10T20:00:00Z'),
  a('Cendre', 12, '2026-09-02T20:00:00Z'),
  a('Dague', 45, '2026-11-01T20:00:00Z'),
  a('Secret', 3, null, { hidden: true, description: 'Tue le boss caché' }),
];

describe('achievement summary', () => {
  it('sorts unlocked achievements by rarity', () => {
    expect(rarity(4.9)).toBe('ultra');
    expect(rarity(5)).toBe('rare');
    expect(rarity(20)).toBe('common');
    expect(rarity(null)).toBe('common');
    expect(rarityCounts(list)).toEqual({ ultra: 1, rare: 1, common: 1 });
    expect(rarest(list)?.apiName).toBe('Brume');
  });

  it('counts unlocks per month from the first to the last one', () => {
    const bars = monthlyUnlocks(list);
    expect(bars[0]).toEqual({ month: '2019-03', count: 1, recent: false });
    expect(bars[bars.length - 1]).toEqual({ month: '2026-11', count: 1, recent: true });
    expect(bars.filter((b) => b.recent).map((b) => b.month)).toEqual([
      '2026-09',
      '2026-10',
      '2026-11',
    ]);
    expect(bars.find((b) => b.month === '2026-10')?.count).toBe(0);
    expect(monthlyUnlocks([a('x', 1)])).toEqual([]);
  });
});

describe('achievement list', () => {
  it('opens on "À débloquer" while some are left', () => {
    expect(defaultView(list)).toBe('locked');
    expect(defaultView(list.filter((i) => i.unlockedAt))).toBe('all');
  });

  it('sorts by ease, rarity, date and name', () => {
    const names = (s: Parameters<typeof sortAchievements>[1]) =>
      sortAchievements(list, s).map((i) => i.apiName);
    expect(names('easiest').slice(0, 2)).toEqual(['Élan', 'Dague']);
    expect(names('rarest')[0]).toBe('Brume');
    expect(names('date').slice(0, 3)).toEqual(['Dague', 'Cendre', 'Brume']);
    expect(names('name')[0]).toBe('Brume');
  });

  it('filters by view and search, never through a masked description', () => {
    const none = () => false;
    expect(filterAchievements(list, 'unlocked', '', none)).toHaveLength(3);
    expect(filterAchievements(list, 'locked', 'elan', none).map((i) => i.apiName)).toEqual([
      'Élan',
    ]);
    const masked = (i: AchievementItem) => isMasked(i, false, new Set());
    expect(filterAchievements(list, 'all', 'boss', masked)).toEqual([]);
    expect(filterAchievements(list, 'all', 'boss', none)).toHaveLength(1);
    expect(isMasked(list[4]!, false, new Set(['Secret']))).toBe(false);
    expect(isMasked(list[4]!, true, new Set())).toBe(false);
  });
});
