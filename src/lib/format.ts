import { fr } from '../i18n/fr';

/** Compact playtime for cards: "45 min", "12 h", "—" when never played. */
export function formatPlaytimeShort(minutes: number): string {
  if (minutes <= 0) return '—';
  if (minutes < 1) return fr.format.underMinute;
  if (minutes < 60) return fr.format.minutes(Math.floor(minutes));
  return fr.format.hours(Math.floor(minutes / 60));
}

/** Detailed playtime for the panel: "45 min", "12 h 05", "< 1 min", "0 min" (none). */
export function formatPlaytimeLong(minutes: number): string {
  if (minutes > 0 && minutes < 1) return fr.format.underMinute;
  const m = Math.max(0, Math.round(minutes));
  if (m < 60) return fr.format.minutes(m);
  return fr.format.hoursMinutes(Math.floor(m / 60), m % 60);
}

const DAY = 24 * 60 * 60 * 1000;

function startOfDay(d: Date): number {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

/** "aujourd'hui", "hier", "il y a 3 jours", then a date ("12 mars 2025"). */
export function formatLastPlayed(iso: string | null | undefined, now: Date = new Date()): string {
  if (!iso) return fr.format.never;
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return fr.format.never;
  const days = Math.round((startOfDay(now) - startOfDay(date)) / DAY);
  if (days <= 0) return fr.format.today;
  if (days === 1) return fr.format.yesterday;
  if (days < 30) return new Intl.RelativeTimeFormat('fr', { numeric: 'auto' }).format(-days, 'day');
  return new Intl.DateTimeFormat('fr-FR', {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
  }).format(date);
}

/** "155,4 Go", "820 Mo". */
export function formatBytes(bytes: number | null | undefined): string {
  if (!bytes || bytes <= 0) return '—';
  const units = ['o', 'Ko', 'Mo', 'Go', 'To'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = unit >= 3 && value < 100 ? 1 : 0;
  return `${new Intl.NumberFormat('fr-FR', { maximumFractionDigits: digits }).format(value)} ${units[unit]}`;
}

/** Stable dark tint derived from a title, for covers without artwork. */
export function titleTint(title: string): string {
  let hash = 0;
  for (const ch of title) hash = (hash * 31 + ch.codePointAt(0)!) | 0;
  const hue = Math.abs(hash) % 360;
  return `hsl(${hue} 28% 18%)`;
}

/** `<input type="datetime-local">` value for a date, in local time. */
export function toLocalInput(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}
