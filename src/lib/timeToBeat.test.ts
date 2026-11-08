import { describe, expect, it } from 'vitest';
import type { TimeToBeat } from '../api/client';
import { hoursToMinutes, ttbChart, ttbProgress } from './timeToBeat';

const ttb = (main: number | null, extra: number | null, done: number | null): TimeToBeat => ({
  provider: 'igdb',
  externalId: '1',
  mainMinutes: main,
  extraMinutes: extra,
  completionistMinutes: done,
  sampleCount: 13,
  url: null,
  fetchedAt: '2026-11-08T10:00:00Z',
});

describe('ttbChart', () => {
  it('puts the bars and the player marker on one scale', () => {
    const c = ttbChart(ttb(1920, 3000, 6000), 1500)!;
    expect(c.bars.map((b) => [b.key, Math.round(b.percent)])).toEqual([
      ['main', 32],
      ['extra', 50],
      ['completionist', 100],
    ]);
    expect(c.markerPercent).toBe(25);
    expect(c.beyond).toBe(false);
  });

  it('pins the marker to the right edge beyond the scale', () => {
    const c = ttbChart(ttb(1920, null, 3000), 8700)!;
    expect(c.bars).toHaveLength(2);
    expect(c.markerPercent).toBe(100);
    expect(c.beyond).toBe(true);
    expect(ttbChart(ttb(null, null, null), 10)).toBeNull();
  });
});

describe('ttbProgress', () => {
  it('says what is left, or what was passed', () => {
    expect(ttbProgress(ttb(1920, 3000, 6000), 1200)).toEqual({ kind: 'mainLeft', minutes: 720 });
    expect(ttbProgress(ttb(1920, 3000, 6000), 2000)).toEqual({ kind: 'extraLeft', minutes: 1000 });
    expect(ttbProgress(ttb(1920, 3000, 6000), 4000)).toEqual({ kind: 'beyondExtra' });
    expect(ttbProgress(ttb(1920, 3000, 6000), 6000)).toEqual({ kind: 'beyondCompletionist' });
    expect(ttbProgress(ttb(1920, null, null), 2000)).toEqual({ kind: 'beyondMain' });
  });
});

describe('hoursToMinutes', () => {
  it('reads hours with a comma or a dot', () => {
    expect(hoursToMinutes('12,5')).toBe(750);
    expect(hoursToMinutes(' 32 ')).toBe(1920);
    expect(hoursToMinutes('')).toBeNull();
    expect(hoursToMinutes('abc')).toBeUndefined();
    expect(hoursToMinutes('0')).toBeUndefined();
  });
});
