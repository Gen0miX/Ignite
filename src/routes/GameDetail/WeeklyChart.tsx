import type { PlaySession } from '../../api/client';
import { fr } from '../../i18n/fr';
import { formatPlaytimeLong } from '../../lib/format';
import { weeklyMinutes } from '../../lib/sessions';
import styles from './GameDetailPage.module.css';

const dateFmt = new Intl.DateTimeFormat('fr-FR', { day: 'numeric', month: 'short' });

/** One bar per week over 12 weeks; the current week in accent (DESIGN.md › graphiques). */
export function WeeklyChart({ sessions }: { sessions: PlaySession[] }) {
  const bars = weeklyMinutes(sessions, 12);
  const max = Math.max(...bars.map((b) => b.minutes), 0);

  return (
    <section className={styles.section} aria-labelledby="sheet-weeks">
      <h2 id="sheet-weeks" className="section-label">
        {fr.game.last12w}
      </h2>
      {max === 0 ? (
        <p className={styles.muted}>{fr.game.last12wEmpty}</p>
      ) : (
        <ol className={`${styles.chart} ${styles.chartWeeks}`}>
          {bars.map((b) => {
            const label = fr.game.weekLabel(
              dateFmt.format(new Date(`${b.monday}T12:00:00`)),
              formatPlaytimeLong(b.minutes),
            );
            return (
              <li key={b.monday} className={styles.barSlot} title={label} aria-label={label}>
                <span
                  className={`${styles.bar} ${b.isCurrent ? styles.barToday : ''}`}
                  style={{ height: `${b.minutes ? Math.max(4, (b.minutes / max) * 100) : 0}%` }}
                />
              </li>
            );
          })}
        </ol>
      )}
    </section>
  );
}
