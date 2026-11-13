import { useState } from 'react';
import { useGameNews, useOpenLink } from '../../api/hooks';
import { Skeleton, SkeletonGroup } from '../../components/Skeleton/Skeleton';
import { strings } from '../../i18n';
import { formatLastPlayed } from '../../lib/format';
import styles from './NewsSection.module.css';

const t = strings.game;

/** Latest Steam announcements of the game; promotions and press on demand. */
export function NewsSection({ gameId }: { gameId: string }) {
  const news = useGameNews(gameId);
  const open = useOpenLink();
  const [showHidden, setShowHidden] = useState(false);
  const data = news.data;
  const items = data ? (showHidden ? [...data.items, ...data.hidden] : data.items) : [];
  const empty = data && data.items.length === 0 && data.hidden.length === 0;

  return (
    <section className={styles.card} aria-labelledby="sheet-news">
      <h2 id="sheet-news" className="section-label">
        {t.news}
      </h2>
      {news.isPending && (
        <SkeletonGroup label={t.newsLoading} className={styles.list}>
          {[0, 1, 2].map((i) => (
            <span key={i} className={styles.skeletonItem}>
              <Skeleton width={90} height={12} />
              <Skeleton width="80%" height={18} />
              <Skeleton height={14} />
            </span>
          ))}
        </SkeletonGroup>
      )}
      {news.isError && <p className={styles.muted}>{t.newsError}</p>}
      {empty && <p className={styles.muted}>{t.newsEmpty}</p>}
      {items.length > 0 && (
        <ul className={styles.list}>
          {items.map((n) => (
            <li key={n.url}>
              <span className={styles.date}>{formatLastPlayed(n.date)}</span>
              <button type="button" className={styles.title} onClick={() => open.mutate(n.url)}>
                {n.title}
              </button>
              {n.excerpt && <p className={styles.muted}>{n.excerpt}</p>}
            </li>
          ))}
        </ul>
      )}
      {data && data.hidden.length > 0 && (
        <p className={styles.muted}>
          {strings.home.hiddenNews(data.hidden.length)}{' '}
          <button type="button" className={styles.link} onClick={() => setShowHidden(!showHidden)}>
            {showHidden ? strings.home.hideAgain : strings.home.showAnyway}
          </button>
        </p>
      )}
    </section>
  );
}
