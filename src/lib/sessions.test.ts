import type { PlaySession } from '../api/client';
import { cardProgress, dailyMinutes, formatClock, sessionStats } from './sessions';

const session = (
  id: string,
  startedAt: string | null,
  minutes: number,
  origin: PlaySession['origin'] = 'auto',
): PlaySession => ({
  id,
  gameId: 'g',
  gameSourceId: null,
  startedAt,
  endedAt: null,
  durationSeconds: minutes * 60,
  idleSeconds: 0,
  origin,
  note: null,
  rating: null,
});

describe('dailyMinutes', () => {
  const now = new Date(2026, 10, 5, 21, 0);

  it('buckets sessions by local start day over 30 days', () => {
    const bars = dailyMinutes(
      [
        session('a', new Date(2026, 10, 5, 20, 0).toISOString(), 45),
        session('b', new Date(2026, 10, 5, 10, 0).toISOString(), 15),
        session('c', new Date(2026, 10, 3, 23, 30).toISOString(), 120),
        session('old', new Date(2026, 8, 1, 12, 0).toISOString(), 300),
        session('est', null, 6000, 'estimate'),
      ],
      30,
      now,
    );
    expect(bars).toHaveLength(30);
    expect(bars[29]).toEqual({ date: '2026-11-05', minutes: 60, isToday: true });
    expect(bars[27]!.minutes).toBe(120);
    expect(bars[0]!.date).toBe('2026-10-07');
    expect(bars.reduce((s, b) => s + b.minutes, 0)).toBe(180);
  });
});

describe('sessionStats', () => {
  it('excludes estimates from the count and average', () => {
    const stats = sessionStats([
      session('a', '2026-11-01T10:00:00Z', 60),
      session('b', '2026-11-02T10:00:00Z', 30, 'manual'),
      session('e', null, 600, 'estimate'),
    ]);
    expect(stats).toEqual({
      count: 2,
      averageMinutes: 45,
      estimateMinutes: 600,
      averageRating: null,
    });
    expect(sessionStats([])).toEqual({
      count: 0,
      averageMinutes: 0,
      estimateMinutes: 0,
      averageRating: null,
    });
    const rated = sessionStats([
      { ...session('r1', '2026-11-01T10:00:00Z', 60), rating: 5 },
      { ...session('r2', '2026-11-02T10:00:00Z', 60), rating: 4 },
      session('r3', '2026-11-03T10:00:00Z', 60),
    ]);
    expect(rated.averageRating).toBe(4.5);
  });
});

describe('formatClock', () => {
  it('formats a live timer', () => {
    expect(formatClock(0)).toBe('0:00');
    expect(formatClock(245)).toBe('4:05');
    expect(formatClock(3723)).toBe('1:02:03');
  });
});

describe('cardProgress', () => {
  it('prefers achievements, then goals', () => {
    expect(cardProgress({ done: 42, total: 42 }, { done: 1, total: 2 })).toEqual({
      kind: 'achievements',
      done: 42,
      total: 42,
      ratio: 1,
    });
    expect(cardProgress({ done: 0, total: 0 }, { done: 6, total: 10 })).toEqual({
      kind: 'goals',
      done: 6,
      total: 10,
      ratio: 0.6,
    });
    expect(cardProgress(null, null)).toBeNull();
  });
});
