import type { GameDetail } from '../../api/client';
import { fr } from '../../i18n/fr';
import styles from './GameDetailPage.module.css';

const t = fr.game;

/** Read-only launch and tracking info; launch profiles come with J3. */
export function LaunchSection({ game }: { game: GameDetail }) {
  return (
    <section className={styles.card} aria-labelledby="sheet-launch">
      <h2 id="sheet-launch" className="section-label">
        {t.launch}
      </h2>
      {game.sources.map((src) => (
        <dl key={src.id} className={styles.details}>
          <dt>{t.launchSource}</dt>
          <dd>{fr.sources[src.kind]}</dd>
          <dt>{t.launchTarget}</dt>
          <dd className="mono">{src.launchUri ?? '—'}</dd>
          <dt>{t.installPath}</dt>
          <dd className="mono">
            {src.installed ? (src.installPath ?? '—') : fr.panel.notInstalled}
          </dd>
          <dt>{t.tracked}</dt>
          <dd>
            {src.installed && src.installPath ? t.trackedPrefix(src.installPath) : t.trackedNone}
          </dd>
        </dl>
      ))}
      <p className={styles.muted}>{t.launchEditSoon}</p>
    </section>
  );
}
