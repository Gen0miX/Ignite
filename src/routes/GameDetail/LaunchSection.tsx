import type { GameDetail } from '../../api/client';
import { InstallFolder } from '../../components/InstallFolder/InstallFolder';
import { strings } from '../../i18n';
import { folderName } from '../../lib/format';
import styles from './GameDetailPage.module.css';

const t = strings.game;

/** Read-only launch and tracking info; editing launch profiles is not built yet. */
export function LaunchSection({ game }: { game: GameDetail }) {
  return (
    <section className={styles.card} aria-labelledby="sheet-launch">
      <h2 id="sheet-launch" className="section-label">
        {t.launch}
      </h2>
      {game.sources.map((src) => (
        <dl key={src.id} className={styles.details}>
          <dt>{t.launchSource}</dt>
          <dd>{strings.sources[src.kind]}</dd>
          <dt>{t.launchTarget}</dt>
          <dd className="mono">{src.launchUri ?? '—'}</dd>
          <dt>{t.installPath}</dt>
          <dd>
            {!src.installed ? (
              strings.panel.notInstalled
            ) : src.installPath ? (
              <InstallFolder sourceId={src.id} path={src.installPath} />
            ) : (
              '—'
            )}
          </dd>
          <dt>{t.tracked}</dt>
          <dd title={src.installed && src.installPath ? src.installPath : undefined}>
            {src.installed && src.installPath
              ? t.trackedPrefix(folderName(src.installPath))
              : t.trackedNone}
          </dd>
        </dl>
      ))}
      <p className={styles.muted}>{t.launchEditSoon}</p>
    </section>
  );
}
