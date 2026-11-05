import type { AchievementItem } from '../api/client';

/** Locked achievements most players have: the easiest next targets. */
export function closest(items: AchievementItem[], n = 5): AchievementItem[] {
  return items
    .filter((i) => !i.unlockedAt)
    .sort((a, b) => (b.globalPercent ?? -1) - (a.globalPercent ?? -1))
    .slice(0, n);
}
