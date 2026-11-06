import {
  formatBytes,
  formatLastPlayed,
  formatPlaytimeLong,
  formatPlaytimeShort,
  titleTint,
} from './format';

describe('playtime', () => {
  it('formats short values for cards', () => {
    expect(formatPlaytimeShort(0)).toBe('—');
    expect(formatPlaytimeShort(45)).toBe('45 min');
    expect(formatPlaytimeShort(60)).toBe('1 h');
    expect(formatPlaytimeShort(11_190)).toBe('186 h');
  });

  it('formats long values for the panel', () => {
    expect(formatPlaytimeLong(0)).toBe('0 min');
    expect(formatPlaytimeLong(0.5)).toBe('< 1 min');
    expect(formatPlaytimeShort(0.5)).toBe('< 1 min');
    expect(formatPlaytimeShort(45.7)).toBe('45 min');
    expect(formatPlaytimeLong(59)).toBe('59 min');
    expect(formatPlaytimeLong(725)).toBe('12 h 05');
  });
});

describe('formatLastPlayed', () => {
  const now = new Date(2026, 10, 5, 15, 0);

  it('uses relative words for recent dates', () => {
    expect(formatLastPlayed(null, now)).toBe('jamais');
    expect(formatLastPlayed('not a date', now)).toBe('jamais');
    expect(formatLastPlayed(new Date(2026, 10, 5, 1, 0).toISOString(), now)).toBe("aujourd'hui");
    expect(formatLastPlayed(new Date(2026, 10, 4, 23, 0).toISOString(), now)).toBe('hier');
    expect(formatLastPlayed(new Date(2026, 10, 2, 12, 0).toISOString(), now)).toBe(
      'il y a 3 jours',
    );
  });

  it('uses a full date after a month', () => {
    expect(formatLastPlayed(new Date(2025, 2, 12, 12, 0).toISOString(), now)).toBe('12 mars 2025');
  });
});

describe('formatBytes', () => {
  it('uses French units', () => {
    expect(formatBytes(null)).toBe('—');
    expect(formatBytes(820 * 1024 * 1024)).toBe('820 Mo');
    expect(formatBytes(155_359_311_885)).toBe('145 Go');
    expect(formatBytes(1.5 * 1024 ** 3)).toBe('1,5 Go');
  });
});

describe('titleTint', () => {
  it('is stable and dark', () => {
    expect(titleTint('Minecraft')).toBe(titleTint('Minecraft'));
    expect(titleTint('Minecraft')).toMatch(/^hsl\(\d+ 28% 18%\)$/);
  });
});
