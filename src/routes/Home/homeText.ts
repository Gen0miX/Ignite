import type { HomeGame, Tonight } from '../../api/client';
import { fr } from '../../i18n/fr';
import { formatLastPlayedShort, formatPlaytimeLong, formatPlaytimeShort } from '../../lib/format';

const t = fr.home;
const BUDGET: Record<Tonight, number> = { halfHour: 30, hour: 60, evening: 240, coop: 240 };
const DAY = 24 * 60 * 60 * 1000;

/**
 * Why a game is suggested for tonight, from what Ignite knows (status,
 * usual session, co-op, HowLongToBeat, last session): two parts at most,
 * e.g. "En cours · ≈ 19 min / session".
 */
export function tonightReason(g: HomeGame, choice: Tonight, now = new Date()): string {
  const s = g.game;
  const parts: string[] = [];
  if (choice === 'coop' && g.coop) parts.push(t.reasonCoop);
  if (s.playtimeMinutes === 0) {
    parts.push(t.reasonNew);
    if (g.hltbMinutes) parts.push(t.reasonHltb(formatPlaytimeShort(g.hltbMinutes)));
  } else {
    if (s.status === 'playing') parts.push(t.reasonPlaying);
    if (g.avgSessionMinutes) {
      const fits = g.avgSessionMinutes <= BUDGET[choice] && BUDGET[choice] <= 60;
      parts.push(
        fits
          ? t.reasonShort(formatPlaytimeLong(g.avgSessionMinutes))
          : t.perSession(formatPlaytimeLong(g.avgSessionMinutes)),
      );
    }
    if (s.lastPlayed) {
      const days = (now.getTime() - new Date(s.lastPlayed).getTime()) / DAY;
      if (days > 180) parts.push(t.reasonForgotten);
      else if (parts.length < 2) parts.push(t.reasonLast(formatLastPlayedShort(s.lastPlayed, now)));
    }
  }
  return parts.slice(0, 2).join(' · ');
}

/** Label on a pile cover: why it is in the pile. */
export function pileTag(g: HomeGame): string {
  if (g.game.status === 'dropped') return t.tagDropped;
  if (g.game.playtimeMinutes > 0) return t.tagPlayed(formatPlaytimeShort(g.game.playtimeMinutes));
  if (g.coop) return t.tagCoop;
  return t.tagNew;
}

/** Under a pile cover: HowLongToBeat time (left), else "Installé", else nothing. */
export function pileMeta(g: HomeGame): string {
  const played = g.game.playtimeMinutes;
  if (g.hltbMinutes) {
    if (played === 0) return t.hltbTotal(formatPlaytimeShort(g.hltbMinutes));
    if (g.hltbMinutes > played) return t.hltbLeft(formatPlaytimeShort(g.hltbMinutes - played));
  }
  return g.game.installed ? fr.library.installedBadge : '';
}
