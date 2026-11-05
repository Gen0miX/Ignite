import { Link, useParams } from 'react-router-dom';
import { Play, Star } from 'lucide-react';
import { artworkSrc, isTauri, type GameDetail, type GameStatus } from '../../api/client';
import { useGame, useLaunchGame, useSessions, useUpdateGame } from '../../api/hooks';
import { PageMain } from '../../app/AppShell';
import buttons from '../../components/Button/Button.module.css';
import { EmptyState } from '../../components/EmptyState/EmptyState';
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { SegmentedControl } from '../../components/SegmentedControl/SegmentedControl';
import { SourceBadge } from '../../components/SourceBadge/SourceBadge';
import { fr } from '../../i18n/fr';
import { formatPlaytimeLong, titleTint } from '../../lib/format';
import { sessionStats } from '../../lib/sessions';
import { AchievementsSection } from './AchievementsSection';
import { DailyChart } from './DailyChart';
import { GoalsSection } from './GoalsSection';
import { LaunchSection } from './LaunchSection';
import { SessionLog } from './SessionLog';
import styles from './GameDetailPage.module.css';

const t = fr.game;
const STATUS_ORDER: GameStatus[] = ['todo', 'playing', 'done', 'dropped'];

export function GameDetailPage() {
  const { id = '' } = useParams();
  const game = useGame(id);

  if (!isTauri() || game.isError) {
    return (
      <PageMain>
        <PageHeader title={t.title} />
        <EmptyState
          action={
            <Link to="/library" className={buttons.secondary}>
              {t.backToLibrary}
            </Link>
          }
        >
          {t.notFound}
        </EmptyState>
      </PageMain>
    );
  }
  if (!game.data) {
    return (
      <PageMain>
        <p className={styles.muted}>{t.loading}</p>
      </PageMain>
    );
  }
  return <Sheet game={game.data} />;
}

function Sheet({ game }: { game: GameDetail }) {
  const s = game.summary;
  const sessions = useSessions(s.id);
  const update = useUpdateGame(s.id);
  const launch = useLaunchGame();
  const list = sessions.data ?? [];
  const stats = sessionStats(list);
  const banner = artworkSrc(game.hero ?? s.header ?? s.cover);
  const playSource = game.sources.find((src) => src.id === game.playSourceId);
  const steamCounted = game.timeBySource.some((p) => p.fromPlatform);

  return (
    <PageMain>
      <header
        className={styles.hero}
        style={banner ? undefined : { background: titleTint(s.title) }}
      >
        {banner && <img src={banner} alt="" />}
        <div className={styles.heroShade} />
        <div className={styles.heroContent}>
          <div className={styles.badges}>
            {s.sources.map((k) => (
              <SourceBadge key={k} kind={k} />
            ))}
          </div>
          <h1 className={styles.title}>{s.title}</h1>
          <div className={styles.heroActions}>
            <button
              type="button"
              className={`${buttons.primary} ${styles.play}`}
              disabled={!playSource || launch.isPending}
              onClick={() => launch.mutate(s.id)}
            >
              <Play size={20} fill="currentColor" aria-hidden="true" />
              {playSource ? fr.panel.playVia(fr.sources[playSource.kind]) : fr.panel.play}
            </button>
            <button
              type="button"
              className={`${buttons.secondary} ${styles.iconButton}`}
              aria-pressed={s.favorite}
              aria-label={s.favorite ? fr.panel.favoriteRemove : fr.panel.favoriteAdd}
              title={s.favorite ? fr.panel.favoriteRemove : fr.panel.favoriteAdd}
              onClick={() => update.mutate({ favorite: !s.favorite })}
            >
              <Star size={18} fill={s.favorite ? 'currentColor' : 'none'} aria-hidden="true" />
            </button>
          </div>
          {launch.isError && (
            <p role="alert" className={styles.error}>
              {fr.panel.launchError}
            </p>
          )}
        </div>
      </header>

      <div className={styles.figures}>
        <Figure
          label={t.totalTime}
          value={formatPlaytimeLong(s.playtimeMinutes)}
          detail={
            stats.estimateMinutes > 0
              ? t.estimatedPart(formatPlaytimeLong(stats.estimateMinutes))
              : undefined
          }
        />
        <Figure
          label={t.sessions}
          value={String(stats.count)}
          detail={t.sessionsDetail(stats.count, formatPlaytimeLong(stats.averageMinutes))}
        />
        {s.achievements && s.achievements.total > 0 ? (
          <Figure
            label={t.achievements}
            value={`${s.achievements.done}/${s.achievements.total}`}
            detail={`${Math.floor((s.achievements.done / s.achievements.total) * 100)} %`}
          />
        ) : (
          <Figure label={t.goals} value={s.goals ? `${s.goals.done}/${s.goals.total}` : '—'} />
        )}
        <div className={styles.figure}>
          <span className={styles.figureLabel}>{t.status}</span>
          <SegmentedControl
            label={t.status}
            options={STATUS_ORDER.map((v) => ({ value: v, label: fr.statusSingular[v] }))}
            value={s.status}
            onChange={(status) => update.mutate({ status })}
          />
        </div>
      </div>

      <div className={styles.columns}>
        <div className={styles.mainColumn}>
          <DailyChart sessions={list} />
          <SessionLog game={game} sessions={list} steamCounted={steamCounted} />
        </div>
        <div className={styles.sideColumn}>
          <AchievementsSection gameId={s.id} hasSteam={s.sources.includes('steam')} />
          <GoalsSection gameId={s.id} />
          <LaunchSection game={game} />
        </div>
      </div>
    </PageMain>
  );
}

function Figure({ label, value, detail }: { label: string; value: string; detail?: string }) {
  return (
    <div className={styles.figure}>
      <span className={styles.figureLabel}>{label}</span>
      <span className={styles.figureValue}>{value}</span>
      {detail && <span className={styles.figureDetail}>{detail}</span>}
    </div>
  );
}
