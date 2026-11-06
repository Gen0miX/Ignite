import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { useLibraryCounts, useTrackerActive } from '../../api/hooks';
import { useQueryClient } from '@tanstack/react-query';
import type { ActiveGame, GameSummary } from '../../api/client';
import { fr } from '../../i18n/fr';
import { formatClock } from '../../lib/sessions';
import styles from './NowPlaying.module.css';
import { useMenuPrefs } from '../Sidebar/menuPrefs';

/** Seconds played: last value from the backend plus time since it was read. */
function useLiveSeconds(game: ActiveGame, fetchedAt: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);
  return game.activeSeconds + Math.max(0, (now - fetchedAt) / 1000);
}

function titleOf(client: ReturnType<typeof useQueryClient>, gameId: string): string | undefined {
  // Look the title up in any cached library list (no extra request).
  for (const [, data] of client.getQueriesData<GameSummary[]>({ queryKey: ['library'] })) {
    const hit = data?.find((g) => g.id === gameId);
    if (hit) return hit.title;
  }
  return undefined;
}

function Row({ game, fetchedAt }: { game: ActiveGame; fetchedAt: number }) {
  const client = useQueryClient();
  const seconds = useLiveSeconds(game, fetchedAt);
  const title = titleOf(client, game.gameId) ?? fr.tracking.nowPlaying;
  return (
    <Link to={`/game/${game.gameId}`} className={styles.row}>
      <span className={styles.dot} aria-hidden="true" />
      <span className={styles.title}>{title}</span>
      <span
        className={styles.clock}
        aria-label={`${fr.tracking.nowPlaying} depuis ${formatClock(seconds)}`}
      >
        {formatClock(seconds)}
      </span>
    </Link>
  );
}

/** Live timer at the bottom of the sidebar while a game is running. */
export function NowPlaying() {
  const { data, dataUpdatedAt } = useTrackerActive();
  useLibraryCounts(); // keep library data warm for titles
  if (!data || data.length === 0) return null;
  return (
    <section className={styles.box} aria-label={fr.tracking.nowPlaying} role="status">
      <span className="section-label">{fr.tracking.nowPlaying}</span>
      {data.map((g) => (
        <Row key={g.sessionId} game={g} fetchedAt={dataUpdatedAt} />
      ))}
    </section>
  );
}

function RailRow({
  game,
  fetchedAt,
  extra,
}: {
  game: ActiveGame;
  fetchedAt: number;
  extra: number;
}) {
  const client = useQueryClient();
  const seconds = useLiveSeconds(game, fetchedAt);
  const title = titleOf(client, game.gameId) ?? fr.tracking.nowPlaying;
  const clock = formatClock(seconds);
  const collapsed = useMenuPrefs((s) => s.collapsed);
  return (
    <Link
      to={`/game/${game.gameId}`}
      className={`${styles.rail} ${collapsed ? styles.railCompact : ''}`}
      title={`${fr.tracking.nowPlaying} : ${title} · ${clock}`}
      aria-label={`${fr.tracking.nowPlaying} : ${title}, ${clock}`}
    >
      <span className={styles.dot} aria-hidden="true" />
      {!collapsed && <span className={styles.railLabel}>{fr.tracking.inGame}</span>}
      <span className={styles.railClock}>
        {clock}
        {extra > 0 ? ` +${extra}` : ''}
      </span>
    </Link>
  );
}

/** Compact live timer for the navigation rail (visible on every screen). */
export function RailNowPlaying() {
  const { data, dataUpdatedAt } = useTrackerActive();
  const first = data?.[0];
  if (!first) return null;
  return (
    <div role="status">
      <RailRow game={first} fetchedAt={dataUpdatedAt} extra={(data?.length ?? 1) - 1} />
    </div>
  );
}
