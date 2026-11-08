import { Link, Navigate, useParams } from 'react-router-dom';
import { Play, Star } from 'lucide-react';
import { artworkSrc, isTauri, type GameDetail, type GameStatus } from '../../api/client';
import {
  useAchievements,
  useCaptures,
  useGame,
  useLaunchGame,
  useSessions,
  useTimeToBeat,
  useUpdateGame,
} from '../../api/hooks';
import { PageMain } from '../../app/AppShell';
import buttons from '../../components/Button/Button.module.css';
import { EmptyState } from '../../components/EmptyState/EmptyState';
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { SegmentedControl } from '../../components/SegmentedControl/SegmentedControl';
import { GameTitle } from '../../components/GameTitle/GameTitle';
import { SourceBadge } from '../../components/SourceBadge/SourceBadge';
import { SkeletonImage } from '../../components/Skeleton/SkeletonImage';
import { fr } from '../../i18n/fr';
import {
  folderName,
  formatLastPlayed,
  formatPlaytimeLong,
  formatPlaytimeShort,
  titleTint,
} from '../../lib/format';
import { sessionStats } from '../../lib/sessions';
import { useState, type ReactNode } from 'react';
import { ImageIcon } from 'lucide-react';
import { AchievementsSection } from './AchievementsSection';
import { CapturesTab } from './CapturesTab';
import { ArtworkPicker } from '../../components/ArtworkPicker/ArtworkPicker';
import { GameTabs, SHEET_TABS, type SheetTab, type TabItem } from './GameTabs';
import { GoalsSection } from './GoalsSection';
import { LaunchSection } from './LaunchSection';
import { MediaSection } from './MediaSection';
import { NewsSection } from './NewsSection';
import { SessionLog } from './SessionLog';
import { TimeToBeatCard } from './TimeToBeatCard';
import { WeeklyChart } from './WeeklyChart';
import styles from './GameDetailPage.module.css';

const t = fr.game;
const STATUS_ORDER: GameStatus[] = ['todo', 'playing', 'done', 'dropped'];

export function GameDetailPage() {
  const { id = '', tab } = useParams();
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
  return <Sheet game={game.data} tabParam={tab} />;
}

/** Achievements come from Steam, GOG or RetroAchievements. */
const hasPlatformAchievements = (game: GameDetail) =>
  game.summary.sources.some((k) => k === 'steam' || k === 'gog' || k === 'emulator');

function Sheet({ game, tabParam }: { game: GameDetail; tabParam: string | undefined }) {
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

  const platformAchievements = hasPlatformAchievements(game);
  // Loaded with the sheet, so the icons download while you look at Aperçu.
  useAchievements(s.id, platformAchievements);
  const ach = s.achievements && s.achievements.total > 0 ? s.achievements : null;
  const goals = s.goals && s.goals.total > 0 ? s.goals : null;
  const captures = useCaptures(s.id).data?.items.length;
  const tabs: TabItem[] = [
    { id: 'apercu', label: t.tabs.overview },
    ...(platformAchievements
      ? [
          {
            id: 'succes' as const,
            label: t.tabs.achievements,
            count: ach ? `${ach.done}/${ach.total}` : undefined,
          },
        ]
      : goals
        ? [{ id: 'succes' as const, label: t.tabs.goals, count: `${goals.done}/${goals.total}` }]
        : []),
    { id: 'captures', label: t.tabs.captures, count: captures ? String(captures) : undefined },
    { id: 'actus', label: t.tabs.news },
    { id: 'lancement', label: t.tabs.launch },
  ];
  const current = (tabParam ?? 'apercu') as SheetTab;
  if (!SHEET_TABS.includes(current) || !tabs.some((x) => x.id === current)) {
    return <Navigate to={`/game/${s.id}`} replace />;
  }

  const playButton = (big: boolean) => (
    <button
      type="button"
      className={`${buttons.primary} ${big ? styles.play : styles.playCompact}`}
      disabled={!playSource || launch.isPending}
      onClick={() => launch.mutate(s.id)}
    >
      <Play size={big ? 20 : 16} fill="currentColor" aria-hidden="true" />
      {playSource ? fr.panel.playVia(fr.sources[playSource.kind]) : fr.panel.play}
    </button>
  );

  return (
    <PageMain>
      {current === 'apercu' ? (
        <>
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
                {playButton(true)}
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
                <button
                  type="button"
                  className={buttons.secondary}
                  onClick={() => setPicking(true)}
                >
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
            <ToFinish gameId={s.id} playedMinutes={s.playtimeMinutes} />
            {ach ? (
              <Figure
                label={t.achievements}
                value={`${ach.done}/${ach.total}`}
                detail={`${Math.floor((ach.done / ach.total) * 100)} %`}
              />
            ) : (
              <Figure label={t.goals} value={goals ? `${goals.done}/${goals.total}` : '—'} />
            )}
            <div className={styles.figure}>
              <span className={styles.figureLabel}>{t.status}</span>
              <SegmentedControl
                label={t.status}
                options={STATUS_ORDER.map((v) => ({
                  value: v,
                  label: fr.statusShort[v],
                  ariaLabel: fr.statusSingular[v],
                }))}
                value={s.status}
                onChange={(status) => update.mutate({ status })}
              />
            </div>
          </div>
        </>
      ) : (
        <header
          className={`${styles.compact} img-host`}
          style={banner ? undefined : { background: titleTint(s.title) }}
        >
          {banner && <SkeletonImage src={banner} />}
          <div className={styles.compactShade} />
          <div className={styles.compactContent}>
            <GameTitle as="h1" title={s.title} logo={game.logo} className={styles.compactTitle} />
            <span className={styles.compactMeta}>
              {t.compactMeta(formatPlaytimeShort(s.playtimeMinutes), fr.statusSingular[s.status])}
            </span>
            {playButton(false)}
          </div>
        </header>
      )}

      <GameTabs gameId={s.id} tabs={tabs} current={current} />

      {current === 'apercu' && (
        <div className={styles.columns}>
          <div className={styles.mainColumn}>
            {game.resumeNote && (
              <section className={styles.resumeCard} aria-labelledby="sheet-resume">
                <h2 id="sheet-resume" className={styles.resumeLabel}>
                  {t.resumeNote}
                  {game.resumeNoteAt && ` · ${formatLastPlayed(game.resumeNoteAt)}`}
                </h2>
                <p className={styles.resume}>{game.resumeNote}</p>
              </section>
            )}
            <WeeklyChart sessions={list} />
            <SessionLog game={game} sessions={list} steamCounted={steamCounted} limit={5} />
            <MediaSection gameId={s.id} />
          </div>
          <div className={styles.sideColumn}>
            <TimeToBeatCard gameId={s.id} playedMinutes={s.playtimeMinutes} />
            <GoalsSection gameId={s.id} />
            <LaunchSummary game={game} />
          </div>
        </div>
      )}
      {current === 'succes' &&
        (platformAchievements ? (
          <AchievementsSection
            gameId={s.id}
            sourceLabel={
              s.sources.includes('emulator')
                ? 'RetroAchievements'
                : s.sources.includes('steam')
                  ? fr.sources.steam
                  : fr.sources.gog
            }
            isEmulated={s.sources.includes('emulator')}
          />
        ) : (
          <GoalsSection gameId={s.id} />
        ))}
      {current === 'captures' && <CapturesTab game={game} />}
      {current === 'actus' && <NewsSection gameId={s.id} />}
      {current === 'lancement' && <LaunchSection game={game} />}
      {picking && <ArtworkPicker gameId={s.id} title={s.title} onClose={() => setPicking(false)} />}
    </PageMain>
  );
}

/** How the game starts, in short; details in the Lancement tab. */
function LaunchSummary({ game }: { game: GameDetail }) {
  const src = game.sources.find((x) => x.id === game.playSourceId) ?? game.sources[0];
  if (!src) return null;
  return (
    <section className={styles.card} aria-labelledby="sheet-launch-summary">
      <h2 id="sheet-launch-summary" className="section-label">
        {t.launch}
      </h2>
      <p className={styles.launchSummary}>
        {t.launchVia(fr.sources[src.kind])}
        {src.installed && src.installPath ? (
          <span title={src.installPath}> · {folderName(src.installPath)}</span>
        ) : (
          <span> · {fr.panel.notInstalled}</span>
        )}
      </p>
      <Link to={`/game/${game.summary.id}/lancement`} className={styles.linkButton}>
        {t.launchDetails}
      </Link>
    </section>
  );
}

/** "Pour le finir": main story time (IGDB or correction) and what is left. */
function ToFinish({ gameId, playedMinutes }: { gameId: string; playedMinutes: number }) {
  const ttb = useTimeToBeat(gameId);
  const main = ttb.data?.best?.mainMinutes ?? null;
  if (main == null) {
    return (
      <Figure label={t.toFinish} value="—" detail={ttb.isPending ? undefined : t.toFinishUnknown} />
    );
  }
  return (
    <Figure
      label={t.toFinish}
      value={`≈ ${formatPlaytimeShort(main)}`}
      detail={
        playedMinutes < main
          ? t.toFinishLeft(`≈ ${formatPlaytimeShort(main - playedMinutes)}`)
          : t.toFinishPassed
      }
    />
  );
}

function Figure({
  label,
  value,
  detail,
  extra,
}: {
  label: string;
  value: string;
  detail?: string;
  extra?: ReactNode;
}) {
  return (
    <div className={styles.figure}>
      <span className={styles.figureLabel}>{label}</span>
      <span className={styles.figureValue}>{value}</span>
      {detail && <span className={styles.figureDetail}>{detail}</span>}
      {extra && <span className={styles.figureDetail}>{extra}</span>}
    </div>
  );
}
