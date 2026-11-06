// Heat calendar (GitHub-like): weeks as columns, Monday first, 4 levels.

export interface CalendarCell {
  /** YYYY-MM-DD, or null for padding before the first day. */
  day: string | null;
  minutes: number;
  level: 0 | 1 | 2 | 3;
}

function iso(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** Level thresholds from the played days: <= 1st third = 1, <= 2nd third = 2, else 3. */
export function thresholds(values: number[]): [number, number] {
  const sorted = values.filter((v) => v > 0).sort((a, b) => a - b);
  if (sorted.length === 0) return [0, 0];
  const at = (q: number) => sorted[Math.floor(q * (sorted.length - 1))]!;
  return [at(1 / 3), at(2 / 3)];
}

/**
 * Weeks (columns of 7 cells, Monday → Sunday) from `start` to `end`
 * inclusive (`YYYY-MM-DD`), with the level of each day.
 */
export function buildCalendar(
  days: { day: string; minutes: number }[],
  start: string,
  end: string,
): CalendarCell[][] {
  const minutes = new Map(days.map((d) => [d.day, d.minutes]));
  const [t1, t2] = thresholds(days.map((d) => d.minutes));
  const level = (m: number): CalendarCell['level'] => (m <= 0 ? 0 : m <= t1 ? 1 : m <= t2 ? 2 : 3);

  const first = new Date(`${start}T12:00:00`);
  const last = new Date(`${end}T12:00:00`);
  const weeks: CalendarCell[][] = [];
  let week: CalendarCell[] = [];
  const offset = (first.getDay() + 6) % 7; // Monday = 0
  for (let i = 0; i < offset; i += 1) week.push({ day: null, minutes: 0, level: 0 });
  for (const d = new Date(first); d <= last; d.setDate(d.getDate() + 1)) {
    const key = iso(d);
    const m = minutes.get(key) ?? 0;
    week.push({ day: key, minutes: m, level: level(m) });
    if (week.length === 7) {
      weeks.push(week);
      week = [];
    }
  }
  if (week.length) weeks.push(week);
  return weeks;
}

/** Month labels: index of the first week containing the 1st of each month. */
export function monthStarts(weeks: CalendarCell[][]): { week: number; month: number }[] {
  const out: { week: number; month: number }[] = [];
  weeks.forEach((w, i) => {
    const first = w.find((c) => c.day?.endsWith('-01'));
    if (first?.day) out.push({ week: i, month: Number(first.day.slice(5, 7)) - 1 });
  });
  return out;
}

/** Calendar cell size filling `width` with `weeks` columns, kept within 10–18 px. */
export function calendarCell(width: number, weeks: number, gap = 3): number {
  if (weeks <= 0 || width <= 0) return 14;
  const size = Math.floor((width - (weeks - 1) * gap) / weeks);
  return Math.max(10, Math.min(18, size));
}
