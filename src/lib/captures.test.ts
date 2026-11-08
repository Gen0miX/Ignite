import { describe, expect, it } from 'vitest';
import type { CaptureItem, CaptureSession, CaptureSource } from '../api/client';
import { fileName, filterCaptures, groupCaptures, sourceCounts } from './captures';

const night: CaptureSession = {
  id: 'night',
  startedAt: new Date(2025, 9, 29, 23, 0).toISOString(),
  endedAt: new Date(2025, 9, 30, 1, 30).toISOString(),
  durationSeconds: 9000,
};

const shot = (
  id: string,
  taken: Date,
  source: CaptureSource = 'steam',
  session: CaptureSession | null = null,
): CaptureItem => ({
  capture: {
    id,
    gameId: 'g',
    source,
    path: String.raw`C:\Steam\userdata\1\760\remote\392110\screenshots\ `.trim() + `${id}.jpg`,
    kind: 'image',
    takenAt: taken.toISOString(),
    width: 2560,
    height: 1440,
    bytes: 431618,
    caption: null,
    thumbPath: null,
  },
  session,
});

// Newest first, as the backend sends them.
const items = [
  shot('c', new Date(2025, 9, 31, 12, 0), 'gamebar'),
  shot('b2', new Date(2025, 9, 30, 0, 40), 'steam', night),
  shot('b1', new Date(2025, 9, 29, 23, 15), 'steam', night),
  shot('a2', new Date(2025, 9, 28, 20, 0), 'folder'),
  shot('a1', new Date(2025, 9, 28, 9, 0), 'steam'),
];

describe('capture groups', () => {
  it('keeps a session past midnight together, else groups by day', () => {
    const groups = groupCaptures(items);
    expect(groups.map((g) => g.items.map((i) => i.capture.id))).toEqual([
      ['c'],
      ['b2', 'b1'],
      ['a2', 'a1'],
    ]);
    expect(groups[1]!.session?.id).toBe('night');
    expect(groups[1]!.date.getDate()).toBe(29);
    expect(groups[2]!.session).toBeNull();
  });

  it('counts and filters by source', () => {
    expect(sourceCounts(items)).toEqual({ all: 5, steam: 3, gamebar: 1, folder: 1 });
    expect(filterCaptures(items, 'gamebar').map((i) => i.capture.id)).toEqual(['c']);
    expect(filterCaptures(items, 'all')).toHaveLength(5);
  });

  it('shows the file name of a Windows path', () => {
    expect(fileName(items[1]!.capture.path)).toBe('b2.jpg');
  });
});
