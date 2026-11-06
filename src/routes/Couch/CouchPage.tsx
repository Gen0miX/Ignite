import { useCallback, useEffect, useRef, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { Gamepad2, X as Close } from 'lucide-react';
import {
  artworkSrc,
  commands,
  isTauri,
  unwrap,
  type HomeGame,
  type Mood,
  type Shelf,
} from '../../api/client';
import { useLaunchGame } from '../../api/hooks';
import { Cover } from '../../components/GameCard/Cover';
import { fr } from '../../i18n/fr';
import {
  formatLastPlayed,
  formatPlaytimeLong,
  formatPlaytimeShort,
  titleTint,
} from '../../lib/format';
import { cycle, step, type PadAction } from '../../lib/gamepad';
import { usePad } from '../../lib/usePad';
import styles from './CouchPage.module.css';

const t = fr.couch;
const SHELVES: Shelf[] = ['recent', 'favorites', 'coop', 'short', 'all'];
const MOODS: Mood[] = ['chill', 'intense', 'coop', 'never'];
/** Delays between two covers while the wheel slows down (ms). */
const SPIN_DELAYS = [50, 50, 55, 60, 70, 80, 95, 115, 140, 170, 210, 260, 320];

function PadKey({ k }: { k: 'A' | 'B' | 'X' | 'Y' | 'LB' | 'RB' }) {
  return (
    <span className={`${styles.key} ${styles[`key${k}`]}`} aria-hidden="true">
      {k}
    </span>
  );
}

function useClock(): string {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 15_000);
    return () => clearInterval(id);
  }, []);
  return new Intl.DateTimeFormat('fr-FR', { hour: '2-digit', minute: '2-digit' }).format(now);
}

/** Couch mode runs fullscreen; the window goes back to normal on exit. */
function useFullscreen() {
  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;
    void import('@tauri-apps/api/window').then(({ getCurrentWindow }) => {
      if (!cancelled) void getCurrentWindow().setFullscreen(true);
    });
    return () => {
      cancelled = true;
      void import('@tauri-apps/api/window').then(({ getCurrentWindow }) =>
        getCurrentWindow().setFullscreen(false),
      );
    };
  }, []);
}

function reducedMotion(): boolean {
  return window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false;
}

interface WheelState {
  mood: number;
  shown: HomeGame | null;
  spinning: boolean;
  done: boolean;
}

function Wheel({
  state,
  candidates,
  loading,
  onMood,
  onSpin,
  onPlay,
  onClose,
}: {
  state: WheelState;
  candidates: number;
  loading: boolean;
  onMood: (i: number) => void;
  onSpin: () => void;
  onPlay: () => void;
  onClose: () => void;
}) {
  const g = state.shown?.game;
  return (
    <div className={styles.overlay}>
      <div role="dialog" aria-modal="true" aria-labelledby="couch-wheel" className={styles.wheel}>
        <div className={styles.wheelHead}>
          <h2 id="couch-wheel" className={styles.wheelTitle}>
            {t.wheel}
          </h2>
          <button type="button" className={styles.iconButton} onClick={onClose}>
            <Close size={22} aria-hidden="true" />
            <span className="visually-hidden">{t.close}</span>
          </button>
        </div>
        <div className={styles.moods} role="group" aria-label={t.moodLabel}>
          {MOODS.map((m, i) => (
            <button
              key={m}
              type="button"
              aria-pressed={i === state.mood}
              className={`${styles.mood} ${i === state.mood ? styles.moodOn : ''}`}
              onClick={() => onMood(i)}
            >
              {t.moods[m]}
            </button>
          ))}
        </div>
        <div className={styles.slot} aria-live="polite">
          {g ? (
            <>
              <span className={`${styles.slotCover} ${state.spinning ? styles.spinning : ''}`}>
                <Cover title={g.title} cover={g.cover} header={g.header} />
              </span>
              <span className={styles.slotTitle}>{state.spinning ? '…' : g.title}</span>
              {state.done && (
                <span className={styles.slotMeta}>
                  {g.playtimeMinutes > 0
                    ? `${formatPlaytimeShort(g.playtimeMinutes)} · ${t.played(formatLastPlayed(g.lastPlayed))}`
                    : t.neverPlayed}
                </span>
              )}
            </>
          ) : (
            <p className={styles.muted}>
              {loading ? t.loading : candidates === 0 ? t.noCandidate : t.wheelReady(candidates)}
            </p>
          )}
        </div>
        <div className={styles.actions}>
          {state.done && (
            <button
              type="button"
              data-pad
              className={styles.primary}
              onClick={onPlay}
              disabled={state.spinning}
            >
              <PadKey k="A" />
              {t.play}
            </button>
          )}
          <button
            type="button"
            className={styles.secondary}
            onClick={onSpin}
            disabled={state.spinning || candidates === 0}
          >
            <PadKey k="X" />
            {state.done ? t.spinAgain : t.spin}
          </button>
        </div>
      </div>
    </div>
  );
}

function Details({ g }: { g: HomeGame }) {
  const s = g.game;
  return (
    <dl className={styles.details}>
      <div>
        <dt>{t.status}</dt>
        <dd>{fr.statusSingular[s.status]}</dd>
      </div>
      <div>
        <dt>{t.sources}</dt>
        <dd>{s.sources.map((k) => fr.sources[k]).join(', ')}</dd>
      </div>
      <div>
        <dt>{t.playtime}</dt>
        <dd>{formatPlaytimeLong(s.playtimeMinutes)}</dd>
      </div>
      {g.avgSessionMinutes ? (
        <div>
          <dt>{t.avgSession}</dt>
          <dd>{formatPlaytimeLong(g.avgSessionMinutes)}</dd>
        </div>
      ) : null}
      {s.goals && s.goals.total > 0 ? (
        <div>
          <dt>{t.goals}</dt>
          <dd>
            {s.goals.done} / {s.goals.total}
          </dd>
        </div>
      ) : null}
    </dl>
  );
}

export function CouchPage() {
  useFullscreen();
  const navigate = useNavigate();
  const launch = useLaunchGame();
  const clock = useClock();
  const [shelf, setShelf] = useState(0);
  const [index, setIndex] = useState(0);
  const [details, setDetails] = useState(false);
  const [wheelOpen, setWheelOpen] = useState(false);
  const [wheel, setWheel] = useState<WheelState>({
    mood: 0,
    shown: null,
    spinning: false,
    done: false,
  });
  const timers = useRef<number[]>([]);
  const cards = useRef<(HTMLButtonElement | null)[]>([]);

  const list = useQuery({
    queryKey: ['couch-shelf', SHELVES[shelf]],
    queryFn: () => unwrap(commands.couchShelf(SHELVES[shelf]!)),
    enabled: isTauri(),
  });
  const pool = useQuery({
    queryKey: ['couch-wheel', MOODS[wheel.mood]],
    queryFn: () => unwrap(commands.couchWheel(MOODS[wheel.mood]!)),
    enabled: isTauri() && wheelOpen,
  });
  const games = list.data ?? [];
  const current = games[Math.min(index, games.length - 1)];
  const candidates = pool.data ?? [];

  const stopSpin = useCallback(() => {
    for (const id of timers.current) clearTimeout(id);
    timers.current = [];
  }, []);
  useEffect(() => stopSpin, [stopSpin]);

  // Focus follows the selection, so the carousel scrolls with the pad.
  useEffect(() => {
    if (wheelOpen) return;
    const el = cards.current[index];
    el?.focus({ preventScroll: true });
    el?.scrollIntoView?.({ inline: 'center', block: 'nearest', behavior: 'smooth' });
  }, [index, shelf, wheelOpen, games.length]);

  const changeShelf = (delta: number) => {
    setShelf((s) => cycle(s, delta, SHELVES.length));
    setIndex(0);
    setDetails(false);
  };

  const play = (id: string | undefined) => {
    if (id) launch.mutate(id);
  };

  const spin = () => {
    if (candidates.length === 0 || wheel.spinning) return;
    stopSpin();
    const pick = candidates[Math.floor(Math.random() * candidates.length)]!;
    if (reducedMotion() || candidates.length === 1) {
      setWheel((w) => ({ ...w, shown: pick, spinning: false, done: true }));
      return;
    }
    setWheel((w) => ({ ...w, spinning: true, done: false }));
    let at = 0;
    for (const delay of SPIN_DELAYS) {
      at += delay;
      const random = candidates[Math.floor(Math.random() * candidates.length)]!;
      timers.current.push(window.setTimeout(() => setWheel((w) => ({ ...w, shown: random })), at));
    }
    timers.current.push(
      window.setTimeout(
        () => setWheel((w) => ({ ...w, shown: pick, spinning: false, done: true })),
        at + 380,
      ),
    );
  };

  const setMood = (i: number) => {
    stopSpin();
    setWheel({ mood: i, shown: null, spinning: false, done: false });
  };

  const closeWheel = () => {
    stopSpin();
    setWheelOpen(false);
    setWheel((w) => ({ ...w, shown: null, spinning: false, done: false }));
  };

  const exit = () => navigate('/');

  const onAction = (a: PadAction) => {
    if (wheelOpen) {
      if (a === 'b') closeWheel();
      else if (a === 'left') setMood(cycle(wheel.mood, -1, MOODS.length));
      else if (a === 'right') setMood(cycle(wheel.mood, 1, MOODS.length));
      else if (a === 'x') spin();
      else if (a === 'a' && wheel.done) play(wheel.shown?.game.id);
      return;
    }
    switch (a) {
      case 'left':
      case 'right':
        setIndex((i) => step(i, a === 'left' ? -1 : 1, games.length));
        setDetails(false);
        break;
      case 'lb':
      case 'rb':
        changeShelf(a === 'lb' ? -1 : 1);
        break;
      case 'a':
        play(current?.game.id);
        break;
      case 'y':
        setDetails((d) => !d);
        break;
      case 'x':
        setWheelOpen(true);
        break;
      case 'b':
        if (details) setDetails(false);
        else exit();
        break;
      default:
        break;
    }
  };
  const padConnected = usePad(onAction);

  if (!isTauri()) {
    return (
      <main className={styles.page}>
        <h1>{t.title}</h1>
        <p>{fr.library.backendUnavailable}</p>
        <Link to="/">{t.exit}</Link>
      </main>
    );
  }

  const s = current?.game;
  const banner = s ? artworkSrc(s.header ?? s.cover) : undefined;
  const shelfName = t.shelves[SHELVES[shelf]!];

  return (
    <main
      className={styles.page}
      style={s && !banner ? { background: titleTint(s.title) } : undefined}
    >
      {banner && <img className={styles.backdrop} src={banner} alt="" />}
      <div className={styles.shade} />
      <h1 className="visually-hidden">{t.title}</h1>

      <header className={styles.header}>
        <PadKey k="LB" />
        <nav aria-label={t.shelvesLabel} className={styles.shelves}>
          {SHELVES.map((k, i) => (
            <button
              key={k}
              type="button"
              aria-pressed={i === shelf}
              className={`${styles.shelf} ${i === shelf ? styles.shelfOn : ''}`}
              onClick={() => changeShelf(i - shelf)}
            >
              {t.shelves[k]}
            </button>
          ))}
        </nav>
        <PadKey k="RB" />
        <div className={styles.status}>
          <span className={styles.pad}>
            <Gamepad2 size={22} aria-hidden="true" />
            {padConnected ? t.pad : t.noPad}
          </span>
          <span className={styles.clock}>{clock}</span>
          <button type="button" className={styles.exit} onClick={exit}>
            <PadKey k="B" />
            {t.exit}
          </button>
        </div>
      </header>

      {s && current ? (
        <section aria-labelledby="couch-focus" className={styles.focus}>
          <h2 id="couch-focus" className={styles.title}>
            {s.title}
          </h2>
          <p className={styles.meta}>
            <span>{s.lastPlayed ? t.played(formatLastPlayed(s.lastPlayed)) : t.neverPlayed}</span>
            {s.playtimeMinutes > 0 && <span>{formatPlaytimeShort(s.playtimeMinutes)}</span>}
            {s.achievements && s.achievements.total > 0 && (
              <span>{t.achievements(s.achievements.done, s.achievements.total)}</span>
            )}
          </p>
          {details ? (
            <Details g={current} />
          ) : (
            current.avgSessionMinutes && (
              <p className={styles.lead}>{t.avg(formatPlaytimeLong(current.avgSessionMinutes))}</p>
            )
          )}
          <div className={styles.actions}>
            <button type="button" className={styles.primary} onClick={() => play(s.id)}>
              <PadKey k="A" />
              {launch.isPending ? t.launching : t.play}
            </button>
            <button
              type="button"
              className={styles.secondary}
              aria-expanded={details}
              onClick={() => setDetails(!details)}
            >
              <PadKey k="Y" />
              {t.details}
            </button>
            {details && (
              <Link to={`/game/${s.id}`} className={styles.secondary}>
                {t.openSheet}
              </Link>
            )}
          </div>
        </section>
      ) : (
        <section className={styles.focus}>
          <p className={styles.lead}>{list.isPending ? t.loading : t.empty}</p>
        </section>
      )}

      <aside aria-labelledby="couch-undecided" className={styles.undecided}>
        <button type="button" className={styles.undecidedHead} onClick={() => setWheelOpen(true)}>
          <PadKey k="X" />
          <span id="couch-undecided">{t.undecided}</span>
        </button>
        <p className={styles.muted}>{t.wheelHint}</p>
        <div className={styles.moodList}>
          {MOODS.map((m) => (
            <span key={m}>{t.moods[m]}</span>
          ))}
        </div>
      </aside>

      <section aria-label={t.carousel} className={styles.carousel}>
        <h2 className={styles.carouselTitle}>{t.count(shelfName, games.length)}</h2>
        <ul className={styles.row}>
          {games.map((g, i) => (
            <li key={g.game.id}>
              <button
                type="button"
                data-pad
                ref={(el) => {
                  cards.current[i] = el;
                }}
                aria-current={i === index ? 'true' : undefined}
                className={`${styles.card} ${i === index ? styles.cardOn : ''}`}
                onClick={() => setIndex(i)}
                onDoubleClick={() => play(g.game.id)}
                onFocus={() => setIndex(i)}
              >
                <Cover title={g.game.title} cover={g.game.cover} header={g.game.header} />
                <span className="visually-hidden">{g.game.title}</span>
              </button>
            </li>
          ))}
        </ul>
      </section>

      <footer className={styles.footer}>
        <span>
          <PadKey k="A" />
          {t.play}
        </span>
        <span>
          <PadKey k="Y" />
          {t.details}
        </span>
        <span>
          <PadKey k="X" />
          {t.wheel}
        </span>
        <span>
          <PadKey k="B" />
          {t.back}
        </span>
        <span className={styles.keyboard}>{t.keyboardHint}</span>
      </footer>

      {wheelOpen && (
        <Wheel
          state={wheel}
          candidates={candidates.length}
          loading={pool.isPending}
          onMood={setMood}
          onSpin={spin}
          onPlay={() => play(wheel.shown?.game.id)}
          onClose={closeWheel}
        />
      )}
    </main>
  );
}
