import { useState } from 'react';
import { Lock, RefreshCw, Trophy } from 'lucide-react';
import type { AchievementItem } from '../../api/client';
import { useAchievements, useRefreshAchievements } from '../../api/hooks';
import buttons from '../../components/Button/Button.module.css';
import { fr } from '../../i18n/fr';
import { formatLastPlayed } from '../../lib/format';
import styles from './GameDetailPage.module.css';

const t = fr.game;
const pct = (p: number) =>
  `${new Intl.NumberFormat('fr-FR', { maximumFractionDigits: 1 }).format(p)} %`;

/** Locked achievements most players have: the easiest next targets. */
export function closest(items: AchievementItem[], n = 5): AchievementItem[] {
  return items
    .filter((i) => !i.unlockedAt)
    .sort((a, b) => (b.globalPercent ?? -1) - (a.globalPercent ?? -1))
    .slice(0, n);
}

function Item({ a }: { a: AchievementItem }) {
  const unlocked = !!a.unlockedAt;
  return (
    <li className={`${styles.achievement} ${unlocked ? '' : styles.locked}`}>
      <span className={styles.achievementIcon} aria-hidden="true">
        {unlocked ? <Trophy size={16} /> : <Lock size={14} />}
      </span>
      <span className={styles.achievementText}>
        <span className={styles.achievementName}>{a.name}</span>
        {a.description && <span className={styles.subtle}>{a.description}</span>}
      </span>
      <span className={styles.achievementMeta}>
        {unlocked
          ? t.unlockedOn(formatLastPlayed(a.unlockedAt))
          : a.globalPercent != null
            ? t.rarity(pct(a.globalPercent))
            : ''}
      </span>
    </li>
  );
}

export function AchievementsSection({ gameId, hasSteam }: { gameId: string; hasSteam: boolean }) {
  const q = useAchievements(gameId, hasSteam);
  const refresh = useRefreshAchievements(gameId);
  const [showAll, setShowAll] = useState(false);
  const v = q.data;

  let body;
  if (!hasSteam || v?.state === 'unsupported') {
    body = <p className={styles.muted}>{t.achievementsUnsupported}</p>;
  } else if (q.isPending) {
    body = <p className={styles.muted}>{t.achievementsLoading}</p>;
  } else if (q.isError || !v) {
    body = <p className={styles.error}>{t.achievementsError}</p>;
  } else if (v.state === 'needsKey') {
    body = <p className={styles.muted}>{t.achievementsNeedsKey}</p>;
  } else if (v.state === 'none') {
    body = <p className={styles.muted}>{t.achievementsNone}</p>;
  } else {
    const next = closest(v.items);
    const all = [...v.items].sort(
      (a, b) =>
        Number(!!b.unlockedAt) - Number(!!a.unlockedAt) ||
        (b.unlockedAt ?? '').localeCompare(a.unlockedAt ?? ''),
    );
    body = (
      <>
        <div className={styles.progressRow}>
          <span className="mono">
            {v.unlocked}/{v.total}
          </span>
          <div className={styles.progressBar} aria-hidden="true">
            <span style={{ width: `${v.total ? (v.unlocked / v.total) * 100 : 0}%` }} />
          </div>
        </div>
        {next.length > 0 && (
          <>
            <h3 className={styles.subheading} title={t.achievementsClosestHint}>
              {t.achievementsClosest}
            </h3>
            <ul className={styles.achievements}>
              {next.map((a) => (
                <Item key={a.apiName} a={a} />
              ))}
            </ul>
          </>
        )}
        <button
          type="button"
          className={styles.linkButton}
          aria-expanded={showAll}
          onClick={() => setShowAll(!showAll)}
        >
          {showAll ? t.achievementsHide : t.achievementsAll(v.total)}
        </button>
        {showAll && (
          <ul className={styles.achievements}>
            {all.map((a) => (
              <Item key={a.apiName} a={a} />
            ))}
          </ul>
        )}
      </>
    );
  }

  return (
    <section className={styles.card} aria-labelledby="sheet-achievements">
      <div className={styles.sectionHeader}>
        <h2 id="sheet-achievements" className="section-label">
          {t.achievementsTitle}
        </h2>
        {hasSteam && v && v.state !== 'needsKey' && (
          <button
            type="button"
            className={`${buttons.secondary} ${styles.smallButton}`}
            onClick={() => refresh.mutate()}
            disabled={refresh.isPending}
          >
            <RefreshCw size={14} aria-hidden="true" />
            {t.achievementsRefresh}
          </button>
        )}
      </div>
      {v?.error && <p className={styles.error}>{t.achievementsError}</p>}
      {body}
    </section>
  );
}
