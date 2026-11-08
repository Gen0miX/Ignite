import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Link } from 'react-router-dom';
import { ChevronLeft, ChevronRight, X } from 'lucide-react';
import { artworkSrc, type CaptureItem, type GameDetail } from '../../api/client';
import buttons from '../../components/Button/Button.module.css';
import { fr } from '../../i18n/fr';
import { dayFormat, fileName, timeFormat } from '../../lib/captures';
import { formatBytes } from '../../lib/format';
import type { useCaptureCommands } from './CapturesTab';
import styles from './CaptureViewer.module.css';

const c = fr.game.cap;

/**
 * Full-window viewer: ← → move between captures (disabled at the ends),
 * Escape closes, focus stays inside and goes back to the opener.
 */
export function CaptureViewer({
  game,
  items,
  index,
  onIndex,
  onClose,
  commands,
}: {
  game: GameDetail;
  items: CaptureItem[];
  index: number;
  onIndex: (i: number) => void;
  onClose: () => void;
  commands: ReturnType<typeof useCaptureCommands>;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const item = items[index]!;
  const cap = item.capture;
  const title = game.summary.title;
  const first = index === 0;
  const last = index === items.length - 1;

  // Latest values for the key handler, registered once.
  const nav = useRef({ index, count: items.length, onIndex, onClose });
  useEffect(() => {
    nav.current = { index, count: items.length, onIndex, onClose };
  });

  useEffect(() => {
    const restore = document.activeElement as HTMLElement | null;
    ref.current?.querySelector<HTMLElement>('[data-close]')?.focus();
    const onKey = (e: KeyboardEvent) => {
      const n = nav.current;
      const typing = (e.target as HTMLElement | null)?.closest('input, textarea');
      if (e.key === 'Escape') {
        e.stopPropagation();
        n.onClose();
      } else if (!typing && e.key === 'ArrowLeft' && n.index > 0) {
        e.preventDefault();
        n.onIndex(n.index - 1);
      } else if (!typing && e.key === 'ArrowRight' && n.index < n.count - 1) {
        e.preventDefault();
        n.onIndex(n.index + 1);
      } else if (e.key === 'Tab' && ref.current) {
        const focusable = [
          ...ref.current.querySelectorAll<HTMLElement>(
            'button:not([disabled]), a[href], input, video[controls]',
          ),
        ];
        const head = focusable[0];
        const tail = focusable[focusable.length - 1];
        if (e.shiftKey && document.activeElement === head) {
          e.preventDefault();
          tail?.focus();
        } else if (!e.shiftKey && document.activeElement === tail) {
          e.preventDefault();
          head?.focus();
        }
      }
    };
    window.addEventListener('keydown', onKey, true);
    return () => {
      window.removeEventListener('keydown', onKey, true);
      restore?.focus?.({ preventScroll: true });
    };
  }, []);

  const taken = new Date(cap.takenAt);
  const src = artworkSrc(cap.path);
  const logo = artworkSrc(game.logo);
  const session = item.session;

  return createPortal(
    <div
      ref={ref}
      role="dialog"
      aria-modal="true"
      aria-label={c.viewer(title)}
      className={styles.viewer}
    >
      <div className={styles.stage}>
        {cap.kind === 'image'
          ? src && <img className={styles.media} src={src} alt={c.alt(title, cap.caption)} />
          : src && (
              <video
                key={cap.id}
                className={styles.media}
                src={src}
                controls
                aria-label={c.alt(title, cap.caption)}
              />
            )}
        <button
          type="button"
          className={`${styles.arrow} ${styles.prev}`}
          aria-label={c.prev}
          disabled={first}
          onClick={() => onIndex(index - 1)}
        >
          <ChevronLeft size={20} aria-hidden="true" />
        </button>
        <button
          type="button"
          className={`${styles.arrow} ${styles.next}`}
          aria-label={c.next}
          disabled={last}
          onClick={() => onIndex(index + 1)}
        >
          <ChevronRight size={20} aria-hidden="true" />
        </button>
        <span className={styles.position} aria-live="polite">
          {c.position(index + 1, items.length)}
        </span>
      </div>

      <aside className={styles.panel}>
        <div className={styles.panelHead}>
          {logo ? (
            <img className={styles.logo} src={logo} alt={title} />
          ) : (
            <span className={styles.titleText}>{title}</span>
          )}
          <button
            type="button"
            data-close
            className={styles.close}
            aria-label={c.close}
            onClick={onClose}
          >
            <X size={16} aria-hidden="true" />
          </button>
        </div>
        <dl className={styles.facts}>
          <dt>{c.date}</dt>
          <dd>
            {dayFormat.format(taken)} · {timeFormat.format(taken)}
          </dd>
          {session && (
            <>
              <dt>{c.session}</dt>
              <dd>
                {c.sessionRange(
                  timeFormat.format(new Date(session.startedAt)),
                  timeFormat.format(new Date(session.endedAt)),
                )}{' '}
                · <Link to={`/game/${game.summary.id}`}>{c.sessionLink}</Link>
              </dd>
            </>
          )}
          <dt>{c.sourceLabel}</dt>
          <dd>
            <span className={styles.badge} data-source={cap.source}>
              {c.badge[cap.source]}
            </span>{' '}
            {c.sourceLong[cap.source]}
          </dd>
          <dt>{c.size}</dt>
          <dd className={styles.mono}>
            {cap.width && cap.height ? `${cap.width}×${cap.height} · ` : ''}
            {formatBytes(cap.bytes)}
          </dd>
          <dt>{c.file}</dt>
          <dd className={`${styles.mono} ${styles.file}`} title={cap.path}>
            {fileName(cap.path)}
          </dd>
        </dl>
        <div className={styles.buttons}>
          {cap.kind === 'image' && (
            <button
              type="button"
              className={`${buttons.primary} ${styles.main}`}
              disabled={commands.actions.useAsHero.isPending}
              onClick={() => commands.useAsHero(cap.id)}
            >
              {c.useAsHero}
            </button>
          )}
          {cap.kind === 'image' && (
            <button
              type="button"
              className={buttons.secondary}
              onClick={() => commands.copy(cap.id)}
            >
              {c.copy}
            </button>
          )}
          <button
            type="button"
            className={buttons.secondary}
            onClick={() => commands.reveal(cap.id)}
          >
            {c.reveal}
          </button>
          <button type="button" className={buttons.secondary} onClick={() => commands.open(cap.id)}>
            {c.openDefault}
          </button>
        </div>
        <Caption key={cap.id} id={cap.id} initial={cap.caption} commands={commands} />
      </aside>
    </div>,
    document.body,
  );
}

/** Saved when leaving the field or with Enter. */
function Caption({
  id,
  initial,
  commands,
}: {
  id: string;
  initial: string | null;
  commands: ReturnType<typeof useCaptureCommands>;
}) {
  const [value, setValue] = useState(initial ?? '');
  const [saved, setSaved] = useState(initial ?? null);
  const [done, setDone] = useState(false);
  const save = () => {
    const next = value.trim() || null;
    if (next === saved) return;
    commands.actions.caption.mutate(
      { id, caption: next },
      {
        onSuccess: () => {
          setSaved(next);
          setDone(true);
        },
      },
    );
  };
  return (
    <div className={styles.caption}>
      <label htmlFor={`caption-${id}`} className="section-label">
        {c.caption}
      </label>
      <input
        id={`caption-${id}`}
        className={styles.input}
        value={value}
        placeholder={c.captionPlaceholder}
        maxLength={200}
        onChange={(e) => {
          setValue(e.target.value);
          setDone(false);
        }}
        onBlur={save}
        onKeyDown={(e) => {
          if (e.key === 'Enter') save();
        }}
      />
      {done && (
        <span className={styles.saved} role="status">
          {c.captionSaved}
        </span>
      )}
    </div>
  );
}
