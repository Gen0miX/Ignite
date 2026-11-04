import type { PlaySession, Progress } from '../api/client';

export interface DayBar {
  /** Local date, YYYY-MM-DD. */
  date: string;
  minutes: number;
  isToday: boolean;
}

function localDay(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/**
 * Minutes played per local day over the last `days` days (oldest first).
 * A session counts on the day it started; estimates (no date) are ignored.
 */
export function dailyMinutes(sessions: PlaySession[], days = 30, now = new Date()): DayBar[] {
  const today = localDay(now);
  const bars: DayBar[] = [];
  const index = new Map<string, number>();
  for (let i = days - 1; i >= 0; i -= 1) {
    const d = new Date(now.getFullYear(), now.getMonth(), now.getDate() - i);
    const key = localDay(d);
    index.set(key, bars.length);
    bars.push({ date: key, minutes: 0, isToday: key === today });
  }
  for (const s of sessions) {
    if (!s.startedAt) continue;
    const i = index.get(localDay(new Date(s.startedAt)));
    if (i !== undefined) bars[i]!.minutes += Math.round(s.durationSeconds / 60);
  }
  return bars;
}

export interface SessionStats {
  /** Dated sessions (estimates excluded). */
  count: number;
  averageMinutes: number;
  /** Minutes entered as "already played". */
  estimateMinutes: number;
}

export function sessionStats(sessions: PlaySession[]): SessionStats {
  const dated = sessions.filter((s) => s.origin !== 'estimate');
  const total = dated.reduce((sum, s) => sum + s.durationSeconds, 0);
  const estimate = sessions
    .filter((s) => s.origin === 'estimate')
    .reduce((sum, s) => sum + s.durationSeconds, 0);
  return {
    count: dated.length,
    averageMinutes: dated.length ? Math.round(total / dated.length / 60) : 0,
    estimateMinutes: Math.round(estimate / 60),
  };
}

/** Live timer: "4:05" under an hour, "1:02:03" above. */
export function formatClock(totalSeconds: number): string {
  const s = Math.max(0, Math.floor(totalSeconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = String(s % 60).padStart(2, '0');
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${sec}` : `${m}:${sec}`;
}

/** Progress shown on cards: "79 %" for achievements, else "objectifs 6/10". */
export function progressLabel(
  achievements: Progress | null | undefined,
  goals: Progress | null | undefined,
): string | null {
  if (achievements && achievements.total > 0) {
    return `${Math.floor((achievements.done / achievements.total) * 100)} %`;
  }
  if (goals && goals.total > 0) return `objectifs ${goals.done}/${goals.total}`;
  return null;
}
