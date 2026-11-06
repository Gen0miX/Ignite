import {
  useEffect,
  useEffectEvent,
  useId,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from 'react';
import { createPortal } from 'react-dom';
import { useNavigate } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Flame, Trophy, X } from 'lucide-react';
import { artworkSrc, commands, unwrap, type GameStatus, type SessionRecap } from '../../api/client';
import { fr } from '../../i18n/fr';
import { formatLastPlayed, formatPlaytimeLong, formatPlaytimeShort } from '../../lib/format';
import { usePrefs } from '../../lib/prefs';
import buttons from '../Button/Button.module.css';
import { Dialog } from '../Dialog/Dialog';
import { SegmentedControl } from '../SegmentedControl/SegmentedControl';
import { Skeleton, SkeletonGroup } from '../Skeleton/Skeleton';
import { SkeletonImage } from '../Skeleton/SkeletonImage';
import { recapKey } from './useRecapEvents';
import styles from './SessionRecapModal.module.css';

const t = fr.recap;
const FLAME_LABELS = ['Bof', 'Correcte', 'Bonne', 'Très bonne', 'Excellente'] as const;
/** Under this share of players, an achievement is "rare". */
const RARE_PERCENT = 15;

const time = new Intl.DateTimeFormat('fr-FR', { hour: '2-digit', minute: '2-digit' });
const pct = (p: number) =>
  `${new Intl.NumberFormat('fr-FR', { maximumFractionDigits: 1 }).format(p)} %`;
const clock = (iso: string) => time.format(new Date(iso));
const seconds = (a: string, b: string) => (new Date(b).getTime() - new Date(a).getTime()) / 1000;

/** Status choices: "À faire" only while it is the current status. */
export function statusChoices(current: GameStatus): GameStatus[] {
  const base: GameStatus[] = ['playing', 'done', 'dropped'];
  return current === 'todo' ? ['todo', ...base] : base;
}

/** End-of-session recap (docs/SESSION-RECAP.md), opened by SessionRecapHost. */
export function SessionRecapModal({
  sessionId,
  onDone,
}: {
  sessionId: string;
  onDone: () => void;
}) {
  const recap = useQuery({
    queryKey: recapKey(sessionId),
    queryFn: () => unwrap(commands.sessionRecap(sessionId)),
  });
  const titleId = useId();
  const ref = useRef<HTMLDivElement>(null);

  // Escape while loading or on error: nothing to lose.
  useEffect(() => {
    if (recap.data) return;
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onDone();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [recap.data, onDone]);

  return createPortal(
    <div className={styles.overlay}>
      <div
        ref={ref}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className={styles.dialog}
        onKeyDown={(e) => trapFocus(e, ref.current)}
      >
        {recap.data ? (
          <RecapContent recap={recap.data} titleId={titleId} onDone={onDone} />
        ) : (
          <div className={styles.loading}>
            <h2 id={titleId} className="visually-hidden">
              {t.loading}
            </h2>
            {recap.isError ? (
              <>
                <p className={styles.muted}>{t.loadError}</p>
                <button type="button" className={buttons.secondary} onClick={onDone}>
                  {t.close}
                </button>
              </>
            ) : (
              <SkeletonGroup label={t.loading}>
                <Skeleton height={200} radius={0} />
                <Skeleton height={12} />
                <Skeleton height={80} />
              </SkeletonGroup>
            )}
          </div>
        )}
      </div>
    </div>,
    document.body,
  );
}

/** Tab stays inside the modal. */
function trapFocus(e: ReactKeyboardEvent, root: HTMLElement | null) {
  if (e.key !== 'Tab' || !root) return;
  const items = [
    ...root.querySelectorAll<HTMLElement>(
      'button:not(:disabled), input:not(:disabled), textarea, select, a[href], [tabindex="0"]',
    ),
  ];
  const first = items[0];
  const last = items[items.length - 1];
  if (!first || !last) return;
  if (e.shiftKey && document.activeElement === first) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && document.activeElement === last) {
    e.preventDefault();
    first.focus();
  }
}

function RecapContent({
  recap,
  titleId,
  onDone,
}: {
  recap: SessionRecap;
  titleId: string;
  onDone: () => void;
}) {
  const client = useQueryClient();
  const navigate = useNavigate();
  const recapMode = usePrefs((p) => p.recapMode);
  const recapMinMinutes = usePrefs((p) => p.recapMinMinutes);
  const setRecap = usePrefs((p) => p.setRecap);
  const initialNote = recap.resumeNote ?? '';
  const [note, setNote] = useState(initialNote);
  const [rating, setRating] = useState<number>(recap.rating ?? 0);
  // A session just happened: "À faire" becomes "En cours" (saved with Save).
  const [status, setStatus] = useState<GameStatus>(
    recap.status === 'todo' ? 'playing' : recap.status,
  );
  const [confirm, setConfirm] = useState<'close' | 'discard' | null>(null);
  const noteRef = useRef<HTMLTextAreaElement>(null);
  const dirty = note.trim() !== initialNote.trim();
  const g = recap.game;
  const sessionId = recap.session.id;

  const refreshAll = () => void client.invalidateQueries();
  const save = useMutation({
    mutationFn: () =>
      unwrap(
        commands.sessionRecapSave(sessionId, {
          resumeNote: note,
          rating,
          status: status === recap.status ? null : status,
        }),
      ),
    onSuccess: refreshAll,
  });
  const discard = useMutation({
    mutationFn: () => unwrap(commands.sessionDiscard(sessionId)),
    onSuccess: () => {
      refreshAll();
      onDone();
    },
  });

  const saveAndClose = () => save.mutate(undefined, { onSuccess: onDone });
  const saveAndOpen = () =>
    save.mutate(undefined, {
      onSuccess: () => {
        onDone();
        navigate(`/game/${g.id}`);
      },
    });
  const close = () => (dirty ? setConfirm('close') : onDone());

  useEffect(() => {
    noteRef.current?.focus();
  }, []);

  // Escape closes, Ctrl+Enter saves (not while a confirmation is open).
  const onShortcut = useEffectEvent((e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      close();
    } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      saveAndClose();
    }
  });
  useEffect(() => {
    if (confirm) return;
    window.addEventListener('keydown', onShortcut);
    return () => window.removeEventListener('keydown', onShortcut);
  }, [confirm]);

  const start = recap.session.startedAt ?? '';
  const end = recap.session.endedAt ?? start;
  const hero = artworkSrc(g.hero ?? g.header);
  const logo = artworkSrc(g.logo);
  const avg = recap.averageSeconds;
  const diffMin = avg != null ? Math.round((recap.activeSeconds - avg) / 60) : 0;
  const hideShortChecked = recapMode === 'longer';
  const hideShortMinutes = recapMode === 'longer' ? recapMinMinutes : 15;

  return (
    <>
      <div className={`${styles.banner} img-host`}>
        {hero && <SkeletonImage src={hero} className={styles.bannerImage} />}
        <div className={styles.bannerShade} />
        <button type="button" className={styles.close} aria-label={t.close} onClick={close}>
          <X size={16} aria-hidden="true" />
        </button>
        <div className={styles.bannerContent}>
          <div>
            <h2 id={titleId} className={styles.kicker}>
              <span className="visually-hidden">{t.dialogLabel(g.title)} · </span>
              {t.range(formatLastPlayed(start), clock(start), clock(end))}
            </h2>
            {logo ? (
              <img src={logo} alt={g.title} className={styles.logo} />
            ) : (
              <p className={styles.gameTitle}>{g.title}</p>
            )}
          </div>
          <div className={styles.duration}>
            <div className={styles.durationValue}>
              {formatPlaytimeLong(recap.activeSeconds / 60)}
            </div>
            {avg != null && (
              <div className={styles.durationNote}>
                {diffMin === 0
                  ? t.sameAsAverage
                  : t.vsAverage(formatPlaytimeLong(Math.abs(diffMin)), diffMin > 0)}
              </div>
            )}
          </div>
        </div>
      </div>

      <Timeline recap={recap} />

      <div className={styles.columns}>
        <div className={styles.column}>
          <div className={styles.tiles}>
            <Tile
              label={t.total}
              value={`${recap.totalApprox ? '≈ ' : ''}${formatPlaytimeShort(recap.totalMinutes)}`}
              detail={t.added(formatPlaytimeLong(recap.activeSeconds / 60))}
              detailClass={styles.ok}
              title={recap.totalApprox ? t.approxHint : formatPlaytimeLong(recap.totalMinutes)}
            />
            <Tile
              label={t.week}
              value={formatPlaytimeLong(recap.week.minutes)}
              detail={t.weekSessions(recap.week.sessions)}
            />
            <Tile
              label={t.streak}
              value={t.streakDays(recap.streak.current)}
              detail={t.streakBest(recap.streak.best)}
            />
          </div>
          <Achievements recap={recap} />
          <Goals recap={recap} onSeeAll={saveAndOpen} />
        </div>

        <div className={styles.column}>
          <section className={styles.section}>
            <label htmlFor="recap-note" className="section-label">
              {t.resume}
            </label>
            <textarea
              ref={noteRef}
              id="recap-note"
              className={styles.note}
              value={note}
              maxLength={2000}
              placeholder={t.resumePlaceholder}
              onChange={(e) => setNote(e.target.value)}
              aria-describedby="recap-note-hint"
            />
            <span id="recap-note-hint" className={styles.hint}>
              {t.resumeHint}
            </span>
          </section>
          <Flames value={rating} onChange={setRating} />
          <section className={styles.section}>
            <h3 id="recap-status" className="section-label">
              {t.status}
            </h3>
            <SegmentedControl
              label={t.status}
              options={statusChoices(recap.status).map((v) => ({
                value: v,
                label: fr.statusShort[v],
                ariaLabel: fr.statusSingular[v],
              }))}
              value={status}
              onChange={setStatus}
            />
          </section>
        </div>
      </div>

      <footer className={styles.footer}>
        <label className={styles.checkbox}>
          <input
            type="checkbox"
            checked={hideShortChecked}
            onChange={(e) => setRecap(e.target.checked ? 'longer' : 'always', hideShortMinutes)}
          />
          {t.hideShort(hideShortMinutes)}
        </label>
        <button type="button" className={styles.discard} onClick={() => setConfirm('discard')}>
          {t.discard}
        </button>
        <span className={styles.shortcuts}>{t.shortcuts}</span>
        <div className={styles.footerActions}>
          {save.isError && (
            <span role="alert" className={styles.error}>
              {t.saveError}
            </span>
          )}
          <button
            type="button"
            className={`${buttons.secondary} ${styles.footerButton}`}
            disabled={save.isPending}
            onClick={saveAndOpen}
          >
            {t.openSheet}
          </button>
          <button
            type="button"
            className={`${buttons.primary} ${styles.footerButton}`}
            disabled={save.isPending}
            onClick={saveAndClose}
          >
            {save.isPending ? t.saving : t.save}
          </button>
        </div>
      </footer>

      {confirm === 'close' && (
        <Dialog
          role="alertdialog"
          title={t.confirmCloseTitle}
          closeLabel={t.cancel}
          onClose={() => setConfirm(null)}
        >
          <p>{t.confirmCloseBody}</p>
          <div className={styles.confirmActions}>
            <button type="button" className={buttons.secondary} onClick={() => setConfirm(null)}>
              {t.keepEditing}
            </button>
            <button type="button" className={buttons.primary} onClick={onDone}>
              {t.closeAnyway}
            </button>
          </div>
        </Dialog>
      )}
      {confirm === 'discard' && (
        <Dialog
          role="alertdialog"
          title={t.discardTitle}
          closeLabel={t.cancel}
          onClose={() => setConfirm(null)}
        >
          <p>{t.discardBody(formatPlaytimeLong(recap.activeSeconds / 60))}</p>
          <div className={styles.confirmActions}>
            <button type="button" className={buttons.secondary} onClick={() => setConfirm(null)}>
              {t.cancel}
            </button>
            <button
              type="button"
              className={`${buttons.primary} ${styles.danger}`}
              disabled={discard.isPending}
              onClick={() => discard.mutate()}
            >
              {t.discardConfirm}
            </button>
          </div>
        </Dialog>
      )}
    </>
  );
}

function Tile({
  label,
  value,
  detail,
  detailClass,
  title,
}: {
  label: string;
  value: string;
  detail: string;
  detailClass?: string;
  title?: string;
}) {
  return (
    <div className={styles.tile} title={title}>
      <span className={styles.tileLabel}>{label}</span>
      <span className={styles.tileValue}>{value}</span>
      <span className={`${styles.tileDetail} ${detailClass ?? ''}`}>{detail}</span>
    </div>
  );
}

/** Active time in accent, removed idle time in `--color-active`. */
function Timeline({ recap }: { recap: SessionRecap }) {
  const segments = recap.timeline;
  const start = recap.session.startedAt ?? '';
  const end = recap.session.endedAt ?? start;
  const idle = segments.filter((s) => s.idle);
  const idleSecs = idle.reduce((sum, s) => sum + seconds(s.start, s.end), 0);
  if (segments.length === 0) return null;
  return (
    <div className={styles.timeline}>
      <div
        className={styles.timelineBar}
        role="img"
        aria-label={t.timelineLabel(
          formatPlaytimeLong(recap.activeSeconds / 60),
          formatPlaytimeLong(idleSecs / 60),
        )}
      >
        {segments.map((s) => (
          <span
            key={s.start}
            className={s.idle ? styles.segmentIdle : styles.segmentActive}
            style={{ flexGrow: Math.max(1, seconds(s.start, s.end)) }}
          />
        ))}
      </div>
      <div className={styles.timelineLabels} aria-hidden="true">
        <span>{clock(start)}</span>
        {idle.map((s) => (
          <span key={s.start}>
            {t.pause(clock(s.start), formatPlaytimeLong(seconds(s.start, s.end) / 60))}
          </span>
        ))}
        <span>{clock(end)}</span>
      </div>
    </div>
  );
}

function Achievements({ recap }: { recap: SessionRecap }) {
  const a = recap.achievements;
  if (!a) return null;
  const head =
    a.total > 0 ? t.achievementsHead(a.done, a.total, Math.round((a.done / a.total) * 100)) : null;
  return (
    <section className={styles.section} aria-labelledby="recap-ach">
      <div className={styles.sectionHead}>
        <h3 id="recap-ach" className="section-label">
          {a.unlocked.length > 0 ? t.achievementsCount(a.unlocked.length) : t.achievements}
        </h3>
        {head && a.unlocked.length > 0 && <span className={styles.mono}>{head}</span>}
      </div>
      {a.syncing && a.unlocked.length === 0 ? (
        <SkeletonGroup label={t.achievementsSyncing} className={styles.achGrid}>
          <Skeleton height={60} radius="var(--radius-lg)" />
          <Skeleton height={60} radius="var(--radius-lg)" />
        </SkeletonGroup>
      ) : a.unlocked.length === 0 ? (
        <p className={styles.muted}>{t.noAchievement(a.done, a.total)}</p>
      ) : (
        <ul className={styles.achGrid}>
          {a.unlocked.map((u) => {
            const rare = u.globalPercent != null && u.globalPercent < RARE_PERCENT;
            return (
              <li key={u.name} className={styles.ach}>
                <AchievementIcon src={u.icon} />
                <span className={styles.achText}>
                  <span className={styles.achName}>{u.name}</span>
                  {u.globalPercent != null && (
                    <span className={rare ? styles.rare : styles.muted}>
                      {rare ? t.rare(pct(u.globalPercent)) : t.players(pct(u.globalPercent))}
                    </span>
                  )}
                </span>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

function AchievementIcon({ src }: { src: string | null }) {
  const [failed, setFailed] = useState(false);
  return (
    <span className={`${styles.achIcon} img-host`} aria-hidden="true">
      {src && !failed ? (
        <SkeletonImage src={src} onError={() => setFailed(true)} />
      ) : (
        <Trophy size={20} />
      )}
    </span>
  );
}

function Goals({ recap, onSeeAll }: { recap: SessionRecap; onSeeAll: () => void }) {
  const client = useQueryClient();
  const [done, setDone] = useState<Record<string, boolean>>({});
  const [title, setTitle] = useState('');
  const toggle = useMutation({
    mutationFn: ({ id, on }: { id: string; on: boolean }) => unwrap(commands.goalToggle(id, on)),
    onSuccess: () => void client.invalidateQueries({ queryKey: ['goals'] }),
  });
  const add = useMutation({
    mutationFn: () => unwrap(commands.goalAdd(recap.game.id, title)),
    onSuccess: () => {
      setTitle('');
      void client.invalidateQueries({ queryKey: recapKey(recap.session.id) });
      void client.invalidateQueries({ queryKey: ['goals'] });
    },
  });
  return (
    <section className={styles.section} aria-labelledby="recap-goals">
      <h3 id="recap-goals" className="section-label">
        {t.goals}
      </h3>
      {recap.openGoals.length > 0 ? (
        <ul className={styles.goals}>
          {recap.openGoals.map((goal) => (
            <li key={goal.id}>
              <label className={`${styles.goal} ${done[goal.id] ? styles.goalDone : ''}`}>
                <input
                  type="checkbox"
                  checked={done[goal.id] ?? false}
                  onChange={(e) => {
                    const on = e.target.checked;
                    setDone((d) => ({ ...d, [goal.id]: on }));
                    toggle.mutate({ id: goal.id, on });
                  }}
                />
                {goal.title}
              </label>
            </li>
          ))}
        </ul>
      ) : (
        <form
          className={styles.goalAdd}
          onSubmit={(e) => {
            e.preventDefault();
            if (title.trim()) add.mutate();
          }}
        >
          <label htmlFor="recap-goal" className="visually-hidden">
            {t.goalAdd}
          </label>
          <input
            id="recap-goal"
            className={styles.input}
            value={title}
            maxLength={200}
            placeholder={t.goalAdd}
            onChange={(e) => setTitle(e.target.value)}
          />
          <button
            type="submit"
            className={buttons.secondary}
            disabled={!title.trim() || add.isPending}
          >
            {t.goalAddButton}
          </button>
        </form>
      )}
      {recap.openGoalsTotal > recap.openGoals.length && (
        <button type="button" className={styles.link} onClick={onSeeAll}>
          {t.goalsAll(recap.openGoalsTotal)}
        </button>
      )}
    </section>
  );
}

/** 1 to 5 flames: one radio group, a single `aria-checked`, arrows move. */
export function Flames({ value, onChange }: { value: number; onChange: (v: number) => void }) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const onKeyDown = (e: ReactKeyboardEvent, i: number) => {
    const delta =
      e.key === 'ArrowRight' || e.key === 'ArrowUp'
        ? 1
        : e.key === 'ArrowLeft' || e.key === 'ArrowDown'
          ? -1
          : 0;
    if (!delta) return;
    e.preventDefault();
    const next = Math.min(4, Math.max(0, i + delta));
    onChange(next + 1);
    refs.current[next]?.focus();
  };
  return (
    <section className={styles.section}>
      <h3 id="recap-flames" className="section-label">
        {t.rating}
      </h3>
      <div role="radiogroup" aria-labelledby="recap-flames" className={styles.flames}>
        {FLAME_LABELS.map((label, i) => {
          const n = i + 1;
          const checked = value === n;
          return (
            <button
              key={label}
              ref={(el) => {
                refs.current[i] = el;
              }}
              type="button"
              role="radio"
              aria-checked={checked}
              aria-label={`${label} (${t.ratingValue(n)})`}
              title={label}
              tabIndex={checked || (value === 0 && i === 0) ? 0 : -1}
              className={`${styles.flame} ${n <= value ? styles.flameOn : ''}`}
              onClick={() => onChange(checked ? 0 : n)}
              onKeyDown={(e) => onKeyDown(e, i)}
            >
              <Flame size={18} fill="currentColor" aria-hidden="true" />
            </button>
          );
        })}
      </div>
    </section>
  );
}
