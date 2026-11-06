import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { ChevronLeft, ChevronRight, Play } from 'lucide-react';
import type { Screenshot, Trailer } from '../../api/client';
import { useGameMedia } from '../../api/hooks';
import { Dialog } from '../../components/Dialog/Dialog';
import { Skeleton, SkeletonGroup } from '../../components/Skeleton/Skeleton';
import { SkeletonImage } from '../../components/Skeleton/SkeletonImage';
import { fr } from '../../i18n/fr';
import { usePrefs } from '../../lib/prefs';
import styles from './MediaSection.module.css';

const t = fr.game;

/** Steam Store trailers, screenshots and short description of the game. */
export function MediaSection({ gameId }: { gameId: string }) {
  const media = useGameMedia(gameId);
  const showTrailers = usePrefs((p) => p.showTrailers);
  const [trailer, setTrailer] = useState(0);
  const [viewing, setViewing] = useState<number | null>(null);
  const data = media.data;
  const trailers = showTrailers ? (data?.media.trailers ?? []) : [];
  const shots = data?.media.screenshots ?? [];
  const current = trailers[Math.min(trailer, trailers.length - 1)];

  return (
    <section className={styles.card} aria-labelledby="sheet-media">
      <h2 id="sheet-media" className="section-label">
        {t.media}
      </h2>
      {media.isPending && (
        <SkeletonGroup label={t.mediaLoading}>
          {showTrailers && (
            <Skeleton
              className={styles.centered}
              width="min(100%, calc(420px * 16 / 9))"
              aspectRatio="16 / 9"
              radius="var(--radius-lg)"
            />
          )}
          <span className={styles.gallery}>
            {[0, 1, 2, 3].map((i) => (
              <Skeleton key={i} aspectRatio="16 / 9" radius="var(--radius-md)" />
            ))}
          </span>
        </SkeletonGroup>
      )}
      {media.isError && <p className={styles.muted}>{t.mediaError}</p>}
      {data && trailers.length === 0 && shots.length === 0 && (
        <p className={styles.muted}>{data.steamAppid ? t.mediaEmpty : t.mediaNone}</p>
      )}
      {data?.igdbKeysMissing && <p className={styles.subtle}>{t.mediaIgdbHint}</p>}

      {current && <TrailerPlayer key={trailerKey(current)} trailer={current} />}
      {trailers.length > 1 && (
        <ul className={styles.strip} aria-label={t.trailers}>
          {trailers.map((tr, i) => (
            <li key={trailerKey(tr)}>
              <button
                type="button"
                className={`${styles.thumb} img-host ${i === trailer ? styles.thumbActive : ''}`}
                aria-pressed={i === trailer}
                aria-label={t.trailerPick(tr.name)}
                title={tr.name}
                onClick={() => setTrailer(i)}
              >
                <SkeletonImage src={tr.thumbnail} loading="lazy" />
                <Play
                  size={14}
                  fill="currentColor"
                  aria-hidden="true"
                  className={styles.thumbIcon}
                />
              </button>
            </li>
          ))}
        </ul>
      )}

      {shots.length > 0 && (
        <ul className={styles.gallery} aria-label={t.screenshots}>
          {shots.map((s, i) => (
            <li key={s.full}>
              <button
                type="button"
                className={`${styles.shot} img-host`}
                aria-label={t.screenshotOpen(i + 1, shots.length)}
                onClick={() => setViewing(i)}
              >
                <SkeletonImage src={s.thumb} loading="lazy" />
              </button>
            </li>
          ))}
        </ul>
      )}

      {data?.media.description && <About text={data.media.description} />}
      {data?.byTitle &&
        data.steamAppid &&
        !data.fromIgdb &&
        (trailers.length > 0 || shots.length > 0) && (
          <p className={styles.subtle}>{t.mediaByTitle}</p>
        )}
      {data?.fromIgdb && <p className={styles.subtle}>{t.mediaFromIgdb}</p>}

      {viewing !== null && <ScreenshotViewer shots={shots} index={viewing} onIndex={setViewing} />}
    </section>
  );
}

const trailerKey = (tr: Trailer) => tr.hls ?? tr.youtube ?? tr.name;

/** Description folded after 4 lines, with "Lire la suite" when it is longer. */
function About({ text }: { text: string }) {
  const ref = useRef<HTMLParagraphElement>(null);
  const [open, setOpen] = useState(false);
  const [long, setLong] = useState(false);

  useLayoutEffect(() => {
    const el = ref.current;
    if (el && !open) setLong(el.scrollHeight > el.clientHeight + 1);
  }, [text, open]);

  return (
    <div className={styles.about}>
      <h3 className={styles.h3}>{t.about}</h3>
      <p ref={ref} id="sheet-about" className={open ? undefined : styles.clamped}>
        {text}
      </p>
      {(long || open) && (
        <button
          type="button"
          className={styles.more}
          aria-expanded={open}
          aria-controls="sheet-about"
          onClick={() => setOpen((v) => !v)}
        >
          {open ? t.readLess : t.readMore}
        </button>
      )}
    </div>
  );
}

/**
 * Poster first (nothing is streamed until asked), then either a native
 * player fed by hls.js (Steam trailers are HLS only and WebView2 has no
 * native HLS) or the privacy-enhanced YouTube embed (IGDB videos).
 */
function TrailerPlayer({ trailer }: { trailer: Trailer }) {
  const [playing, setPlaying] = useState(false);
  const [failed, setFailed] = useState(false);
  const video = useRef<HTMLVideoElement>(null);
  const hlsUrl = trailer.hls;

  useEffect(() => {
    const el = video.current;
    if (!playing || !el || !hlsUrl) return;
    if (el.canPlayType('application/vnd.apple.mpegurl')) {
      el.src = hlsUrl;
      void el.play().catch(() => undefined);
      return;
    }
    let destroyed = false;
    let hls: { destroy: () => void } | null = null;
    void import('hls.js').then(({ default: Hls }) => {
      if (destroyed) return;
      if (!Hls.isSupported()) {
        setFailed(true);
        return;
      }
      // No worker: the CSP has no worker-src for blob URLs.
      const player = new Hls({ enableWorker: false, capLevelToPlayerSize: true });
      hls = player;
      player.on(Hls.Events.ERROR, (_e, data) => {
        if (data.fatal) setFailed(true);
      });
      player.loadSource(hlsUrl);
      player.attachMedia(el);
      player.on(Hls.Events.MANIFEST_PARSED, () => void el.play().catch(() => undefined));
    });
    return () => {
      destroyed = true;
      hls?.destroy();
    };
  }, [playing, hlsUrl]);

  if (failed) {
    return (
      <div className={styles.player}>
        <SkeletonImage src={trailer.thumbnail} />
        <p className={styles.playerError} role="alert">
          {t.trailerError}
        </p>
      </div>
    );
  }
  if (!playing) {
    return (
      <button
        type="button"
        className={`${styles.player} img-host`}
        aria-label={t.trailerPlay(trailer.name)}
        onClick={() => setPlaying(true)}
      >
        <SkeletonImage src={trailer.thumbnail} />
        <span className={styles.playButton} aria-hidden="true">
          <Play size={28} fill="currentColor" />
        </span>
        <span className={styles.playerName}>{trailer.name}</span>
      </button>
    );
  }
  if (trailer.youtube) {
    return (
      <div className={styles.player}>
        <iframe
          src={`https://www.youtube-nocookie.com/embed/${encodeURIComponent(trailer.youtube)}?autoplay=1&rel=0`}
          title={trailer.name}
          allow="autoplay; encrypted-media; picture-in-picture; fullscreen"
          allowFullScreen
          referrerPolicy="strict-origin-when-cross-origin"
        />
      </div>
    );
  }
  return (
    <div className={styles.player}>
      <video
        ref={video}
        controls
        playsInline
        poster={trailer.thumbnail}
        aria-label={trailer.name}
      />
    </div>
  );
}

function ScreenshotViewer({
  shots,
  index,
  onIndex,
}: {
  shots: Screenshot[];
  index: number;
  onIndex: (i: number | null) => void;
}) {
  const n = shots.length;
  const go = (delta: number) => onIndex((index + delta + n) % n);
  const shot = shots[index];

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'ArrowLeft') onIndex((index - 1 + n) % n);
      if (e.key === 'ArrowRight') onIndex((index + 1) % n);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [index, n, onIndex]);

  if (!shot) return null;
  return (
    <Dialog
      title={t.screenshotTitle(index + 1, n)}
      closeLabel={t.close}
      onClose={() => onIndex(null)}
      width={1280}
    >
      <div className={`${styles.viewer} img-host`}>
        <SkeletonImage src={shot.full} className={styles.viewerImage} />
        {n > 1 && (
          <>
            <button
              type="button"
              className={`${styles.nav} ${styles.navPrev}`}
              aria-label={t.previous}
              onClick={() => go(-1)}
            >
              <ChevronLeft size={22} aria-hidden="true" />
            </button>
            <button
              type="button"
              className={`${styles.nav} ${styles.navNext}`}
              aria-label={t.next}
              onClick={() => go(1)}
            >
              <ChevronRight size={22} aria-hidden="true" />
            </button>
          </>
        )}
      </div>
    </Dialog>
  );
}
