import { describe, expect, it } from 'vitest';
import { defaultStatsRange } from './prefs';

describe('defaultStatsRange', () => {
  const now = new Date(2026, 10, 6, 15, 0);

  it('opens on "since forever" while the tracked history is young', () => {
    expect(defaultStatsRange(null, now)).toBe('all');
    expect(defaultStatsRange('2026-11-06', now)).toBe('all');
    expect(defaultStatsRange('2026-09-10', now)).toBe('all');
    expect(defaultStatsRange('2026-08-01', now)).toBe('year');
  });
});
