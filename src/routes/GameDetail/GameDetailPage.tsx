import { Link, useParams } from 'react-router-dom';
import { Play, Star } from 'lucide-react';
import { artworkSrc, isTauri, type GameDetail, type GameStatus } from '../../api/client';
import { useGame, useLaunchGame, useSessions, useUpdateGame } from '../../api/hooks';
import { PageMain } from '../../app/AppShell';
import buttons from '../../components/Button/Button.module.css';
import { EmptyState } from '../../components/EmptyState/EmptyState';
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { SegmentedControl } from '../../components/SegmentedControl/SegmentedControl';
import { GameTitle } from '../../components/GameTitle/GameTitle';
import { SourceBadge } from '../../components/SourceBadge/SourceBadge';
import { SkeletonImage } from '../../components/Skeleton/SkeletonImage';
import { fr } from '../../i18n/fr';
import { formatPlaytimeLong, titleTint } from '../../lib/format';
import { sessionStats } from '../../lib/sessions';
import { useState } from 'react';
import { ImageIcon } from 'lucide-react';
import { AchievementsSection } from './AchievementsSection';
import { ArtworkPicker } from '../../components/ArtworkPicker/ArtworkPicker';
import { DailyChart } from './DailyChart';
import { GoalsSection } from './GoalsSection';
import { LaunchSection } from './LaunchSection';
import { MediaSection } from './MediaSection';
import { NewsSection } from './NewsSection';
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
  const [picking, setPicking] = useState(false);

  return (
    <PageMain>
      <header
        className={`${styles.hero} img-host`}
        style={banner ? undefined : { background: titleTint(s.title) }}
      >
        {banner && <SkeletonImage src={banner} />}
        <div className={styles.heroShade} />
        <div className={styles.heroContent}>
          <div className={styles.badges}>
            {s.sources.map((k) => (
              <SourceBadge key={k} kind={k} />
            ))}
          </div>
          <GameTitle
            as="h1"
            size="page"
            title={s.title}
            logo={game.logo}
            className={styles.title}
          />
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
            <button type="button" className={buttons.secondary} onClick={() => setPicking(true)}>
              <ImageIcon size={16} aria-hidden="true" />
              {fr.artwork.open}
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
          <MediaSection gameId={s.id} />
        </div>
        <div className={styles.sideColumn}>
          <AchievementsSection
            gameId={s.id}
            supported={s.sources.some((k) => k === 'steam' || k === 'gog' || k === 'emulator')}
            isEmulated={s.sources.includes('emulator')}
          />
          <GoalsSection gameId={s.id} />
          <NewsSection gameId={s.id} />
          <LaunchSection game={game} />
        </div>
      </div>
      {picking && <ArtworkPicker gameId={s.id} title={s.title} onClose={() => setPicking(false)} />}
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
