import { buildCalendar, monthStarts, thresholds } from './calendar';

describe('calendar', () => {
  it('splits played days into three levels', () => {
    expect(thresholds([])).toEqual([0, 0]);
    expect(thresholds([10, 20, 30, 40, 50, 60])).toEqual([20, 40]);
  });

  it('builds Monday-first weeks with padding', () => {
    // 2026-01-01 is a Thursday: 3 padding cells (Mon-Wed).
    const weeks = buildCalendar(
      [
        { day: '2026-01-01', minutes: 30 },
        { day: '2026-01-02', minutes: 120 },
      ],
      '2026-01-01',
      '2026-01-11',
    );
    expect(weeks).toHaveLength(2);
    expect(weeks[0]!.slice(0, 3).every((c) => c.day === null)).toBe(true);
    expect(weeks[0]![3]).toMatchObject({ day: '2026-01-01', minutes: 30 });
    expect(weeks[0]![4]!.level).toBe(3);
    expect(weeks[0]![5]).toMatchObject({ day: '2026-01-03', level: 0 });
    expect(weeks[1]![6]!.day).toBe('2026-01-11');
  });

  it('labels the week of each 1st of month', () => {
    const weeks = buildCalendar([], '2026-01-01', '2026-03-31');
    const starts = monthStarts(weeks);
    expect(starts.map((s) => s.month)).toEqual([0, 1, 2]);
    expect(starts[0]!.week).toBe(0);
  });
});
