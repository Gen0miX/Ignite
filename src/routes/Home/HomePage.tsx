import { useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useMutation, useQuery } from '@tanstack/react-query';
import { Dices, Play } from 'lucide-react';
import {
  artworkSrc,
  commands,
  isTauri,
  unwrap,
  type HomeGame,
  type Tonight,
} from '../../api/client';
import { useGame, useLaunchGame } from '../../api/hooks';
import { PageMain } from '../../app/AppShell';
import buttons from '../../components/Button/Button.module.css';
import { EmptyState } from '../../components/EmptyState/EmptyState';
import { Cover } from '../../components/GameCard/Cover';
import { SegmentedControl } from '../../components/SegmentedControl/SegmentedControl';
import { fr } from '../../i18n/fr';
import {
  formatLastPlayed,
  formatPlaytimeLong,
  formatPlaytimeShort,
  titleTint,
} from '../../lib/format';
import styles from './HomePage.module.css';

const t = fr.home;

function MiniCard({ g, meta }: { g: HomeGame; meta?: string }) {
  return (
    <Link to={`/library?game=${encodeURIComponent(g.game.id)}`} className={styles.mini}>
      <span className={styles.miniCover}>
        <Cover title={g.game.title} cover={g.game.cover} header={g.game.header} />
      </span>
      <span className={styles.miniTitle}>{g.game.title}</span>
      <span className={styles.miniMeta}>{meta ?? formatPlaytimeShort(g.game.playtimeMinutes)}</span>
    </Link>
  );
}

function Resume({ g }: { g: HomeGame }) {
  const launch = useLaunchGame();
  const s = g.game;
  // The wide background image when there is one; wait for it rather than
  // showing the header first and swapping.
  const detail = useGame(s.id);
  const banner = detail.isPending
    ? undefined
    : artworkSrc(detail.data?.hero ?? s.header ?? s.cover);
  return (
    <section
      className={styles.hero}
      aria-labelledby="home-resume"
      style={banner ? undefined : { background: titleTint(s.title) }}
    >
      {banner && <img src={banner} alt="" />}
      <div className={styles.heroShade} />
      <div className={styles.heroBody}>
        <span className={styles.kicker}>{t.resume(formatLastPlayed(s.lastPlayed))}</span>
        <h2 id="home-resume" className={styles.heroTitle}>
          {s.title}
        </h2>
        <div className={styles.heroStats}>
          <span>
            <strong className="mono">{formatPlaytimeShort(s.playtimeMinutes)}</strong> {t.playtime}
          </span>
          {g.avgSessionMinutes ? (
            <span>
              <strong className="mono">{formatPlaytimeLong(g.avgSessionMinutes)}</strong>{' '}
              {t.avgSession}
            </span>
          ) : null}
          {s.achievements && s.achievements.total > 0 ? (
            <span>
              <strong className="mono">
                {s.achievements.done} / {s.achievements.total}
              </strong>{' '}
              {t.achievements}
            </span>
          ) : null}
        </div>
        <div className={styles.heroActions}>
          <button
            type="button"
            className={`${buttons.primary} ${styles.play}`}
            onClick={() => launch.mutate(s.id)}
          >
            <Play size={18} fill="currentColor" aria-hidden="true" />
            {fr.panel.play}
          </button>
          <Link to={`/game/${s.id}`} className={buttons.secondary}>
            {fr.panel.openSheet}
          </Link>
        </div>
      </div>
    </section>
  );
}

function TonightBlock() {
  const navigate = useNavigate();
  const [choice, setChoice] = useState<Tonight>('hour');
  const list = useQuery({
    queryKey: ['home-tonight', choice],
    queryFn: () => unwrap(commands.homeTonight(choice)),
  });
  const games = list.data ?? [];
  const surprise = () => {
    const pick = games[Math.floor(Math.random() * games.length)];
    if (pick) navigate(`/library?game=${encodeURIComponent(pick.game.id)}`);
  };
  return (
    <section className={styles.section} aria-labelledby="home-tonight">
      <div className={styles.sectionHead}>
        <div>
          <h2 id="home-tonight" className={styles.h2}>
            {t.tonight}
          </h2>
          <p className={styles.muted}>{t.tonightHint}</p>
        </div>
        <button
          type="button"
          className={buttons.secondary}
          disabled={games.length === 0}
          onClick={surprise}
        >
          <Dices size={16} aria-hidden="true" />
          {t.surprise}
        </button>
      </div>
      <SegmentedControl
        label={t.tonight}
        options={(['halfHour', 'hour', 'evening', 'coop'] as const).map((v) => ({
          value: v,
          label: t.choices[v],
        }))}
        value={choice}
        onChange={setChoice}
      />
      {games.length === 0 ? (
        <p className={styles.muted}>{list.isPending ? t.loading : t.noSuggestion}</p>
      ) : (
        <div className={styles.row}>
          {games.map((g) => (
            <MiniCard
              key={g.game.id}
              g={g}
              meta={
                g.avgSessionMinutes
                  ? t.perSession(formatPlaytimeLong(g.avgSessionMinutes))
                  : undefined
              }
            />
          ))}
        </div>
      )}
    </section>
  );
}

function NewsBlock() {
  const [showHidden, setShowHidden] = useState(false);
  const news = useQuery({
    queryKey: ['home-news'],
    queryFn: () => unwrap(commands.homeNews()),
    staleTime: 30 * 60_000,
  });
  const open = useMutation({ mutationFn: (url: string) => unwrap(commands.openLink(url)) });
  const items = news.data
    ? showHidden
      ? [...news.data.items, ...news.data.hidden]
      : news.data.items
    : [];
  return (
    <section className={styles.card} aria-labelledby="home-news">
      <h2 id="home-news" className={styles.h2}>
        {t.news}
      </h2>
      <p className={styles.muted}>{t.newsHint}</p>
      {news.isPending && <p className={styles.muted}>{t.loading}</p>}
      {news.isError && <p className={styles.muted}>{t.newsError}</p>}
      {news.data && items.length === 0 && <p className={styles.muted}>{t.noNews}</p>}
      <ul className={styles.news}>
        {items.map((n) => (
          <li key={n.url}>
            <span className={styles.kicker}>
              {n.gameTitle} · {formatLastPlayed(n.date)}
            </span>
            <button type="button" className={styles.newsTitle} onClick={() => open.mutate(n.url)}>
              {n.title}
            </button>
            {n.excerpt && <p className={styles.muted}>{n.excerpt}</p>}
          </li>
        ))}
      </ul>
      {news.data && news.data.hidden.length > 0 && (
        <p className={styles.muted}>
          {t.hiddenNews(news.data.hidden.length)}{' '}
          <button type="button" className={styles.link} onClick={() => setShowHidden(!showHidden)}>
            {showHidden ? t.hideAgain : t.showAnyway}
          </button>
        </p>
      )}
    </section>
  );
}

function WeekBlock() {
  const stats = useQuery({
    queryKey: ['stats', 'week', []],
    queryFn: () => unwrap(commands.statsOverview('week', [])),
  });
  const s = stats.data;
  const today = new Date();
  const monday = new Date(
    today.getFullYear(),
    today.getMonth(),
    today.getDate() - ((today.getDay() + 6) % 7),
  );
  const days = Array.from({ length: 7 }, (_, i) => {
    const d = new Date(monday.getFullYear(), monday.getMonth(), monday.getDate() + i);
    const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
    return {
      key,
      label: t.weekdays[i]!,
      minutes: s?.calendar.find((c) => c.day === key)?.minutes ?? 0,
      today: d.toDateString() === today.toDateString(),
    };
  });
  const max = Math.max(1, ...days.map((d) => d.minutes));
  return (
    <section className={styles.card} aria-labelledby="home-week">
      <h2 id="home-week" className={styles.h2}>
        {t.week}
      </h2>
      {s && (
        <div className={styles.weekStats}>
          <span>
            <strong className="mono">{formatPlaytimeLong(s.totalMinutes)}</strong> {t.played}
          </span>
          <span>
            <strong className="mono">{s.gamesPlayed}</strong> {t.gamesLaunched}
          </span>
          <span>
            <strong className="mono">{s.longestStreakDays} j</strong> {t.streak}
          </span>
        </div>
      )}
      <ol className={styles.weekBars} aria-label={t.week}>
        {days.map((d) => (
          <li key={d.key} title={`${d.label} : ${formatPlaytimeLong(d.minutes)}`}>
            <span className={styles.barTrack}>
              <span
                className={`${styles.bar} ${d.today ? styles.barToday : ''}`}
                style={{ height: `${d.minutes ? Math.max(6, (d.minutes / max) * 100) : 0}%` }}
              />
            </span>
            <span className={styles.barLabel}>{d.label}</span>
          </li>
        ))}
      </ol>
    </section>
  );
}

export function HomePage() {
  const home = useQuery({
    queryKey: ['home'],
    queryFn: () => unwrap(commands.homeOverview()),
    enabled: isTauri(),
  });
  if (!isTauri()) {
    return (
      <PageMain>
        <EmptyState title={t.title}>{fr.library.backendUnavailable}</EmptyState>
      </PageMain>
    );
  }
  const h = home.data;
  return (
    <PageMain>
      <h1 className="visually-hidden">{t.title}</h1>
      {h?.resume ? (
        <Resume g={h.resume} />
      ) : (
        h && (
          <EmptyState
            title={t.emptyTitle}
            action={
              <Link to="/import" className={buttons.primary}>
                {fr.import.title}
              </Link>
            }
          >
            {t.emptyBody}
          </EmptyState>
        )
      )}
      {h && h.recent.length > 0 && (
        <section className={styles.section} aria-labelledby="home-recent">
          <div className={styles.sectionHead}>
            <h2 id="home-recent" className={styles.h2}>
              {t.recent}
            </h2>
            <Link to="/library" className={styles.link}>
              {t.seeAll}
            </Link>
          </div>
          <div className={styles.row}>
            {h.recent.map((g) => (
              <MiniCard key={g.game.id} g={g} meta={formatLastPlayed(g.game.lastPlayed)} />
            ))}
          </div>
        </section>
      )}
      <TonightBlock />
      {h && h.pile.length > 0 && (
        <section className={styles.section} aria-labelledby="home-pile">
          <div className={styles.sectionHead}>
            <div>
              <h2 id="home-pile" className={styles.h2}>
                {t.pile(h.pileTotal)}
              </h2>
              <p className={styles.muted}>{t.pileHint}</p>
            </div>
            <Link to="/library?view=status-todo" className={styles.link}>
              {t.seeAll}
            </Link>
          </div>
          <div className={styles.row}>
            {h.pile.map((g) => (
              <MiniCard
                key={g.game.id}
                g={g}
                meta={g.game.installed ? fr.library.installedBadge : t.notInstalled}
              />
            ))}
          </div>
        </section>
      )}
      <div className={styles.columns}>
        <NewsBlock />
        <WeekBlock />
      </div>
    </PageMain>
  );
}
