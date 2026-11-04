import type { PlaySession } from '../../api/client';
import { fr } from '../../i18n/fr';
import { formatPlaytimeLong } from '../../lib/format';
import { dailyMinutes } from '../../lib/sessions';
import styles from './GameDetailPage.module.css';

const dateFmt = new Intl.DateTimeFormat('fr-FR', { day: 'numeric', month: 'short' });

/** One bar per day over 30 days; today in accent (DESIGN.md › graphiques). */
export function DailyChart({ sessions }: { sessions: PlaySession[] }) {
  const bars = dailyMinutes(sessions, 30);
  const max = Math.max(...bars.map((b) => b.minutes), 0);

  return (
    <section className={styles.section} aria-labelledby="sheet-last30">
      <h2 id="sheet-last30" className="section-label">
        {fr.game.last30}
      </h2>
      {max === 0 ? (
        <p className={styles.muted}>{fr.game.last30Empty}</p>
      ) : (
        <ol className={styles.chart}>
          {bars.map((b) => {
            const label = fr.game.chartLabel(
              dateFmt.format(new Date(`${b.date}T12:00:00`)),
              formatPlaytimeLong(b.minutes),
            );
            return (
              <li key={b.date} className={styles.barSlot} title={label} aria-label={label}>
                <span
                  className={`${styles.bar} ${b.isToday ? styles.barToday : ''}`}
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
