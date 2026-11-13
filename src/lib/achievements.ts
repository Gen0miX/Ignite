import type { AchievementItem } from '../api/client';
import { locale } from '../i18n';

export type Rarity = 'ultra' | 'rare' | 'common';

/** Ultra-rare under 5 % of players, rare from 5 to 20 %, else common. */
export function rarity(percent: number | null | undefined): Rarity {
  if (percent == null) return 'common';
  if (percent < 5) return 'ultra';
  if (percent < 20) return 'rare';
  return 'common';
}

/** Unlocked achievements per rarity. */
export function rarityCounts(items: AchievementItem[]): Record<Rarity, number> {
  const counts: Record<Rarity, number> = { ultra: 0, rare: 0, common: 0 };
  for (const i of items) if (i.unlockedAt) counts[rarity(i.globalPercent)] += 1;
  return counts;
}

export interface MonthBar {
  /** YYYY-MM, local time. */
  month: string;
  count: number;
  /** One of the last 3 months of the range (shown in accent). */
  recent: boolean;
}

const monthKey = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}`;

/** Unlocks per month, from the first unlock to the last one (empty months included). */
export function monthlyUnlocks(items: AchievementItem[]): MonthBar[] {
  const dates = items
    .map((i) => (i.unlockedAt ? new Date(i.unlockedAt) : null))
    .filter((d): d is Date => d !== null && !Number.isNaN(d.getTime()))
    .sort((a, b) => a.getTime() - b.getTime());
  const first = dates[0];
  const last = dates[dates.length - 1];
  if (!first || !last) return [];
  const counts = new Map<string, number>();
  for (const d of dates) counts.set(monthKey(d), (counts.get(monthKey(d)) ?? 0) + 1);
  const bars: MonthBar[] = [];
  const cursor = new Date(first.getFullYear(), first.getMonth(), 1);
  const end = new Date(last.getFullYear(), last.getMonth(), 1);
  while (cursor <= end) {
    const key = monthKey(cursor);
    bars.push({ month: key, count: counts.get(key) ?? 0, recent: false });
    cursor.setMonth(cursor.getMonth() + 1);
  }
  for (const b of bars.slice(-3)) b.recent = true;
  return bars;
}

/** The unlocked achievement with the lowest share of players. */
export function rarest(items: AchievementItem[]): AchievementItem | null {
  let best: AchievementItem | null = null;
  for (const i of items) {
    if (!i.unlockedAt || i.globalPercent == null) continue;
    if (!best || i.globalPercent < (best.globalPercent ?? Infinity)) best = i;
  }
  return best;
}

export type AchView = 'all' | 'locked' | 'unlocked';
export type AchSort = 'easiest' | 'rarest' | 'date' | 'name';

/** "À débloquer" while some are left, else "Tous". */
export function defaultView(items: AchievementItem[]): AchView {
  return items.some((i) => !i.unlockedAt) ? 'locked' : 'all';
}

const byName = (a: AchievementItem, b: AchievementItem) => a.name.localeCompare(b.name, locale);

export function sortAchievements(items: AchievementItem[], sort: AchSort): AchievementItem[] {
  const list = [...items];
  const pct = (i: AchievementItem) => i.globalPercent ?? -1;
  switch (sort) {
    case 'easiest':
      return list.sort((a, b) => pct(b) - pct(a) || byName(a, b));
    case 'rarest':
      return list.sort(
        (a, b) => (a.globalPercent ?? Infinity) - (b.globalPercent ?? Infinity) || byName(a, b),
      );
    case 'date':
      return list.sort(
        (a, b) => (b.unlockedAt ?? '').localeCompare(a.unlockedAt ?? '') || byName(a, b),
      );
    case 'name':
      return list.sort(byName);
  }
}

/** A hidden achievement's description stays secret until unlocked or revealed. */
export const isMasked = (i: AchievementItem, revealAll: boolean, revealed: Set<string>) =>
  i.hidden && !i.unlockedAt && !revealAll && !revealed.has(i.apiName);

const fold = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();

export function filterAchievements(
  items: AchievementItem[],
  view: AchView,
  query: string,
  isHidden: (i: AchievementItem) => boolean,
): AchievementItem[] {
  const q = fold(query.trim());
  return items.filter((i) => {
    if (view === 'locked' && i.unlockedAt) return false;
    if (view === 'unlocked' && !i.unlockedAt) return false;
    if (!q) return true;
    // A masked achievement is only found by what is visible of it.
    if (isHidden(i)) return false;
    return fold(`${i.name} ${i.description ?? ''}`).includes(q);
  });
}
