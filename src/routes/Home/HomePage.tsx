import { useRef, useState, type KeyboardEvent } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useMutation, useQuery } from '@tanstack/react-query';
import { Play, Shuffle, SlidersHorizontal } from 'lucide-react';
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
import { Menu } from '../../components/Menu/Menu';
import { Skeleton, SkeletonGroup } from '../../components/Skeleton/Skeleton';
import { SkeletonImage } from '../../components/Skeleton/SkeletonImage';
import { strings } from '../../i18n';
import {
  formatLastPlayed,
  formatLastPlayedShort,
  formatPlaytimeLong,
  formatPlaytimeShort,
  titleTint,
} from '../../lib/format';
import { pileMeta, pileTag, tonightReason } from './homeText';
import styles from './HomePage.module.css';

const t = strings.home;
const libraryLink = (id: string) => `/library?game=${encodeURIComponent(id)}`;

/** Cover with title and a line under it (recently played, pile). */
function CoverCard({ g, meta, tag }: { g: HomeGame; meta: string; tag?: string }) {
  return (
    <Link to={libraryLink(g.game.id)} className={styles.coverCard}>
      <span className={`${styles.cover} img-host`}>
        <Cover title={g.game.title} cover={g.game.cover} header={g.game.header} />
        {tag && <span className={styles.coverTag}>{tag}</span>}
      </span>
      <span className={styles.coverTitle}>{g.game.title}</span>
      {meta && <span className={styles.coverMeta}>{meta}</span>}
    </Link>
  );
}

/** Big "Reprendre" banner; "Ensuite" swaps the game it shows (no launch). */
function Hero({ games }: { games: HomeGame[] }) {
  const [currentId, setCurrentId] = useState(games[0]?.game.id);
  const g = games.find((x) => x.game.id === currentId) ?? games[0];
  if (!g) return null;
  const next = games.filter((x) => x.game.id !== g.game.id).slice(0, 3);
  return (
    <section className={styles.heroWrap} aria-labelledby="home-resume">
      <HeroGame key={g.game.id} g={g} />
      {next.length > 0 && (
        <div className={styles.next}>
          <span className={styles.nextLabel}>{t.next}</span>
          <ul className={styles.nextList}>
            {next.map((n) => {
              const img = artworkSrc(n.game.header ?? n.hero ?? n.game.cover);
              return (
                <li key={n.game.id}>
                  <button
                    type="button"
                    className={`${styles.nextThumb} img-host`}
                    aria-label={t.showInHero(n.game.title)}
                    title={n.game.title}
                    onClick={() => setCurrentId(n.game.id)}
                    style={img ? undefined : { background: titleTint(n.game.title) }}
                  >
                    {img ? <SkeletonImage src={img} /> : <span>{n.game.title}</span>}
                  </button>
                </li>
              );
            })}
          </ul>
        </div>
      )}
    </section>
  );
}

function HeroGame({ g }: { g: HomeGame }) {
  const launch = useLaunchGame();
  const detail = useGame(g.game.id);
  const [menuAt, setMenuAt] = useState<{ x: number; y: number } | null>(null);
  // Missing files fall back: hero → header → cover, logo → title text.
  const [failed, setFailed] = useState<string[]>([]);
  const fail = (src: string) => setFailed((f) => [...f, src]);
  const s = g.game;
  const banner = [g.hero, s.header, s.cover]
    .map((p) => artworkSrc(p))
    .find((src) => src && !failed.includes(src));
  const logoSrc = artworkSrc(g.logo);
  const logo = logoSrc && !failed.includes(logoSrc) ? logoSrc : undefined;
  const sources = (detail.data?.sources ?? []).filter((src) => src.installed);
  return (
    <div
      className={`${styles.hero} img-host`}
      style={banner ? undefined : { background: titleTint(s.title) }}
    >
      {banner && (
        <SkeletonImage src={banner} className={styles.heroImage} onError={() => fail(banner)} />
      )}
      <div className={styles.heroShade} />
      <div className={styles.heroBody}>
        <span className={styles.heroKicker}>
          <span className={styles.dot} aria-hidden="true" />
          {t.resumeKicker(formatLastPlayed(s.lastPlayed))}
        </span>
        <h2 id="home-resume" className={styles.heroTitle}>
          {logo ? (
            <img src={logo} alt={s.title} className={styles.heroLogo} onError={() => fail(logo)} />
          ) : (
            s.title
          )}
        </h2>
        {g.resumeNote && <p className={styles.heroNote}>{g.resumeNote}</p>}
        <dl className={styles.heroStats}>
          <div>
            <dt>{t.playtime}</dt>
            <dd>{formatPlaytimeShort(s.playtimeMinutes)}</dd>
          </div>
          {g.avgSessionMinutes ? (
            <div>
              <dt>{t.avgSession}</dt>
              <dd>{formatPlaytimeLong(g.avgSessionMinutes)}</dd>
            </div>
          ) : null}
          {s.achievements && s.achievements.total > 0 ? (
            <div>
              <dt>{t.achievements}</dt>
              <dd>
                {s.achievements.done} / {s.achievements.total}
              </dd>
            </div>
          ) : null}
        </dl>
        <div className={styles.heroActions}>
          <button
            type="button"
            className={`${buttons.primary} ${styles.heroPlay}`}
            disabled={launch.isPending}
            onClick={() => launch.mutate(s.id)}
          >
            <Play size={18} fill="currentColor" aria-hidden="true" />
            {strings.panel.play}
          </button>
          <Link to={`/game/${s.id}`} className={styles.heroGhost}>
            {t.sheet}
          </Link>
          {sources.length > 1 && (
            <button
              type="button"
              className={`${styles.heroGhost} ${styles.heroIcon}`}
              aria-label={t.launchOptions}
              title={t.launchOptions}
              aria-haspopup="menu"
              aria-expanded={menuAt !== null}
              onClick={(e) => {
                const r = e.currentTarget.getBoundingClientRect();
                setMenuAt({ x: r.left, y: r.bottom + 6 });
              }}
            >
              <SlidersHorizontal size={20} aria-hidden="true" />
            </button>
          )}
        </div>
        {launch.isError && (
          <p role="alert" className={styles.error}>
            {strings.panel.launchError}
          </p>
        )}
      </div>
      {menuAt && (
        <Menu
          label={t.launchOptions}
          at={menuAt}
          items={sources.map((src) => ({
            label: t.launchVia(strings.sources[src.kind]),
            onSelect: () => void commands.launchGameSource(s.id, src.id),
          }))}
          onClose={() => setMenuAt(null)}
        />
      )}
    </div>
  );
}

function FirstGame() {
  return (
    <section className={`${styles.hero} ${styles.heroEmpty}`} aria-labelledby="home-first">
      <div className={styles.heroBody}>
        <h2 id="home-first" className={styles.heroTitle}>
          {t.firstGameTitle}
        </h2>
        <p className={styles.heroNote}>{t.firstGameBody}</p>
        <div className={styles.heroActions}>
          <Link to="/library" className={`${buttons.primary} ${styles.heroPlay}`}>
            {t.openLibrary}
          </Link>
        </div>
      </div>
    </section>
  );
}

const CHOICES: Tonight[] = ['halfHour', 'hour', 'evening', 'coop'];

/** Time choice as pills: a radio group, arrows move. */
function TonightPills({ value, onChange }: { value: Tonight; onChange: (v: Tonight) => void }) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const onKey = (e: KeyboardEvent, i: number) => {
    const delta = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
    if (!delta) return;
    e.preventDefault();
    const n = (i + delta + CHOICES.length) % CHOICES.length;
    onChange(CHOICES[n]!);
    refs.current[n]?.focus();
  };
  return (
    <div role="radiogroup" aria-label={t.tonight} className={styles.pills}>
      {CHOICES.map((c, i) => (
        <button
          key={c}
          ref={(el) => {
            refs.current[i] = el;
          }}
          type="button"
          role="radio"
          aria-checked={value === c}
          tabIndex={value === c ? 0 : -1}
          className={`${styles.pill} ${value === c ? styles.pillOn : ''}`}
          onClick={() => onChange(c)}
          onKeyDown={(e) => onKey(e, i)}
        >
          {t.choices[c]}
        </button>
      ))}
    </div>
  );
}

function TonightCard() {
  const navigate = useNavigate();
  const launch = useLaunchGame();
  const [choice, setChoice] = useState<Tonight>('hour');
  const list = useQuery({
    queryKey: ['home-tonight', choice],
    queryFn: () => unwrap(commands.homeTonight(choice)),
  });
  const games = list.data ?? [];
  const surprise = () => {
    const pick = games[Math.floor(Math.random() * games.length)];
    if (pick) navigate(libraryLink(pick.game.id));
  };
  return (
    <section className={`${styles.card} ${styles.side}`} aria-labelledby="home-tonight">
      <div>
        <h2 id="home-tonight" className={styles.h2Small}>
          {t.tonight}
        </h2>
        <p className={styles.muted}>{t.tonightCardHint}</p>
      </div>
      <TonightPills value={choice} onChange={setChoice} />
      {list.isPending ? (
        <SkeletonGroup label={t.loading}>
          {[0, 1, 2].map((i) => (
            <Skeleton key={i} height={82} radius="var(--radius-xl)" />
          ))}
        </SkeletonGroup>
      ) : games.length === 0 ? (
        <p className={styles.muted}>{t.noSuggestion}</p>
      ) : (
        <ul className={styles.suggestions}>
          {games.slice(0, 3).map((g) => (
            <li key={g.game.id} className={styles.suggestion}>
              <span className={`${styles.suggestionCover} img-host`}>
                <Cover title={g.game.title} cover={g.game.cover} header={g.game.header} />
              </span>
              <span className={styles.suggestionText}>
                <Link to={libraryLink(g.game.id)} className={styles.suggestionTitle}>
                  {g.game.title}
                </Link>
                <span className={styles.muted}>{tonightReason(g, choice)}</span>
              </span>
              <button
                type="button"
                className={styles.suggestionPlay}
                aria-label={t.launchTitle(g.game.title)}
                title={t.launchTitle(g.game.title)}
                onClick={() => launch.mutate(g.game.id)}
              >
                <Play size={16} fill="currentColor" aria-hidden="true" />
              </button>
            </li>
          ))}
        </ul>
      )}
      <button
        type="button"
        className={styles.surprise}
        disabled={games.length === 0}
        onClick={surprise}
      >
        <Shuffle size={18} aria-hidden="true" />
        {t.surprise}
      </button>
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
  const data = news.data;
  const items = data ? (showHidden ? [...data.items, ...data.hidden] : data.items.slice(0, 2)) : [];
  const hiddenCount = data?.hidden.length ?? 0;
  return (
    <section className={styles.main} aria-labelledby="home-news">
      <div className={styles.sectionHead}>
        <h2 id="home-news" className={styles.h2}>
          {t.news}
        </h2>
        <span className={styles.muted}>{t.newsHint}</span>
      </div>
      {news.isPending ? (
        <SkeletonGroup label={t.loading} className={styles.newsGrid}>
          {[0, 1, 2].map((i) => (
            <Skeleton key={i} height={190} radius="var(--radius-2xl)" />
          ))}
        </SkeletonGroup>
      ) : news.isError ? (
        <p className={styles.muted}>{t.newsError}</p>
      ) : (
        <div className={styles.newsGrid}>
          {items.map((n) => {
            const img = artworkSrc(n.image ?? n.header);
            return (
              <article key={n.url} className={styles.newsCard}>
                <span className={`${styles.newsImage} img-host`}>
                  {img && <SkeletonImage src={img} loading="lazy" />}
                </span>
                <div className={styles.newsBody}>
                  <span className={styles.newsKicker}>
                    {n.patch ? t.newsPatch : t.newsOther} · {formatLastPlayedShort(n.date)}
                  </span>
                  <button
                    type="button"
                    className={styles.newsTitle}
                    onClick={() => open.mutate(n.url)}
                  >
                    {n.gameTitle} — {n.title}
                  </button>
                </div>
              </article>
            );
          })}
          <div className={styles.newsHidden}>
            {hiddenCount > 0 ? (
              <>
                <span className={styles.newsHiddenCount}>{hiddenCount}</span>
                <span>{t.hiddenNewsCount(hiddenCount)}</span>
                <button
                  type="button"
                  className={styles.link}
                  aria-expanded={showHidden}
                  onClick={() => setShowHidden(!showHidden)}
                >
                  {showHidden ? t.hideAgain : t.showAnyway}
                </button>
              </>
            ) : (
              items.length === 0 && <span>{t.noNewsCard}</span>
            )}
          </div>
        </div>
      )}
    </section>
  );
}

function WeekCard() {
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
    <section className={`${styles.card} ${styles.side}`} aria-labelledby="home-week">
      <h2 id="home-week" className={styles.h2Small}>
        {t.week}
      </h2>
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
      {s && (
        <dl className={styles.weekStats}>
          <div>
            <dt>{t.played}</dt>
            <dd>{formatPlaytimeLong(s.totalMinutes)}</dd>
          </div>
          <div>
            <dt>{t.gamesLaunched}</dt>
            <dd>{s.gamesPlayed}</dd>
          </div>
          <div>
            <dt>{t.streak}</dt>
            <dd>{s.longestStreakDays} j</dd>
          </div>
        </dl>
      )}
    </section>
  );
}

function HomeSkeleton() {
  return (
    <SkeletonGroup label={t.loading} className={styles.page}>
      <Skeleton height="max(360px, min(540px, 62vh))" radius="24px" />
      <span className={styles.coverRow}>
        {[0, 1, 2, 3, 4, 5].map((i) => (
          <Skeleton key={i} aspectRatio="2 / 3" radius="var(--radius-xl)" />
        ))}
      </span>
    </SkeletonGroup>
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
        <EmptyState title={t.title}>{strings.library.backendUnavailable}</EmptyState>
      </PageMain>
    );
  }
  const h = home.data;
  if (!h) {
    return (
      <PageMain>
        <h1 className="visually-hidden">{t.title}</h1>
        <HomeSkeleton />
      </PageMain>
    );
  }
  const heroGames = h.resume ? [h.resume, ...h.recent] : [];
  const pileHasHltb = h.pile.some((g) => g.hltbMinutes);
  return (
    <PageMain>
      <div className={styles.page}>
        <h1 className="visually-hidden">{t.title}</h1>
        {heroGames.length > 0 ? <Hero games={heroGames} /> : <FirstGame />}

        <div className={styles.split}>
          {h.recent.length > 0 && (
            <section className={styles.main} aria-labelledby="home-recent">
              <div className={styles.sectionHead}>
                <h2 id="home-recent" className={styles.h2}>
                  {t.recent}
                </h2>
                <Link to="/library?sort=lastPlayed" className={styles.link}>
                  {t.seeAll}
                </Link>
              </div>
              <div className={styles.coverRow}>
                {h.recent.slice(0, 6).map((g) => (
                  <CoverCard
                    key={g.game.id}
                    g={g}
                    meta={t.recentMeta(
                      formatLastPlayedShort(g.game.lastPlayed),
                      formatPlaytimeShort(g.game.playtimeMinutes),
                    )}
                  />
                ))}
              </div>
            </section>
          )}
          <TonightCard />
        </div>

        {h.pile.length > 0 && (
          <section aria-labelledby="home-pile">
            <div className={styles.sectionHead}>
              <h2 id="home-pile" className={styles.h2}>
                {t.pile(h.pileTotal)}
              </h2>
              <span className={styles.muted}>{pileHasHltb ? t.pileHintHltb : t.pileHint}</span>
              <Link to="/library?view=status-todo" className={`${styles.link} ${styles.pushRight}`}>
                {t.seeAll}
              </Link>
            </div>
            <div className={`${styles.coverRow} ${styles.pileRow}`}>
              {h.pile.map((g) => (
                <CoverCard key={g.game.id} g={g} tag={pileTag(g)} meta={pileMeta(g)} />
              ))}
            </div>
          </section>
        )}

        <div className={styles.split}>
          <NewsBlock />
          <WeekCard />
        </div>
      </div>
    </PageMain>
  );
}
