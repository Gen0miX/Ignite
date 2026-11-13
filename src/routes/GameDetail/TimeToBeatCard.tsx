import { useState, type FormEvent } from 'react';
import { Link } from 'react-router-dom';
import { ArrowRight, RefreshCw } from 'lucide-react';
import type { TimeToBeat, TimeToBeatView } from '../../api/client';
import { useOpenLink, useTimeToBeat, useTimeToBeatMutations } from '../../api/hooks';
import buttons from '../../components/Button/Button.module.css';
import { Skeleton, SkeletonGroup } from '../../components/Skeleton/Skeleton';
import { fr } from '../../i18n/fr';
import { formatLastPlayed, formatPlaytimeShort } from '../../lib/format';
import { hoursToMinutes, ttbChart, ttbProgress, type TtbKey } from '../../lib/timeToBeat';
import styles from './TimeToBeatCard.module.css';

const t = fr.game.ttb;
const approx = (minutes: number) => `≈ ${formatPlaytimeShort(minutes)}`;

/** "Durée de vie" (docs/TIME-TO-BEAT.md): IGDB or the user's correction. */
export function TimeToBeatCard({
  gameId,
  playedMinutes,
}: {
  gameId: string;
  playedMinutes: number;
}) {
  const q = useTimeToBeat(gameId);
  const [editing, setEditing] = useState(false);
  const view = q.data;
  const best = view?.best ?? null;

  return (
    <section className={styles.card} aria-labelledby="sheet-ttb">
      <div className={styles.head}>
        <h2 id="sheet-ttb" className="section-label">
          {t.title}
        </h2>
        {best && <span className={styles.provider}>{t.provider[best.provider]}</span>}
      </div>
      {q.isPending ? (
        <SkeletonGroup label={t.loading}>
          {[0, 1, 2].map((i) => (
            <Skeleton key={i} height={18} />
          ))}
        </SkeletonGroup>
      ) : editing && view ? (
        <CorrectForm gameId={gameId} view={view} onDone={() => setEditing(false)} />
      ) : best ? (
        <Chart best={best} played={playedMinutes} />
      ) : (
        <p className={styles.empty}>
          {t.unknown} ·{' '}
          <button type="button" className={styles.link} onClick={() => setEditing(true)}>
            {t.add}
          </button>
        </p>
      )}
      {view?.igdbKeysMissing && !best && (
        <p className={styles.hint}>
          {t.keysMissing}{' '}
          <Link to="/settings/accounts" className={styles.link}>
            {t.openSettings}
          </Link>
        </p>
      )}
      {view && view.candidates.length > 0 && !editing && (
        <p className={styles.hint}>
          {t.doubtful}{' '}
          <button type="button" className={styles.link} onClick={() => setEditing(true)}>
            {t.choose}
          </button>
        </p>
      )}
      {view?.error && <p className={styles.hint}>{t.error}</p>}
      {view && !editing && (best || view.sources.length > 0) && (
        <Footer gameId={gameId} view={view} onCorrect={() => setEditing(true)} />
      )}
    </section>
  );
}

const LABEL: Record<TtbKey, string> = {
  main: t.main,
  extra: t.extra,
  completionist: t.completionist,
};

function Chart({ best, played }: { best: TimeToBeat; played: number }) {
  const chart = ttbChart(best, played);
  const progress = ttbProgress(best, played);
  if (!chart) return null;
  return (
    <>
      <div className={styles.chart}>
        <ul className={styles.bars}>
          {chart.bars.map((b) => (
            <li key={b.key} className={styles.row}>
              <span className={styles.label}>{LABEL[b.key]}</span>
              <span className={styles.track}>
                <span className={styles.fill} style={{ width: `${b.percent}%` }} />
              </span>
              <span className={styles.value}>{approx(b.minutes)}</span>
            </li>
          ))}
        </ul>
        {played > 0 && (
          <div className={styles.markerLane} aria-hidden="true">
            <span
              className={`${styles.marker} ${chart.beyond ? styles.markerBeyond : ''}`}
              style={{ left: `${chart.markerPercent}%` }}
            >
              <span className={styles.markerLabel}>
                {t.you(formatPlaytimeShort(played))}
                {chart.beyond && <ArrowRight size={12} aria-hidden="true" />}
              </span>
            </span>
          </div>
        )}
      </div>
      {progress && (
        <p className={styles.sentence}>
          {progress.kind === 'mainLeft'
            ? t.mainLeft(approx(progress.minutes))
            : progress.kind === 'extraLeft'
              ? t.extraLeft(approx(progress.minutes))
              : t[progress.kind]}
        </p>
      )}
    </>
  );
}

function Footer({
  gameId,
  view,
  onCorrect,
}: {
  gameId: string;
  view: TimeToBeatView;
  onCorrect: () => void;
}) {
  const open = useOpenLink();
  const { refresh } = useTimeToBeatMutations(gameId);
  const igdb = view.sources.find((s) => s.provider === 'igdb');
  const best = view.best;
  const source =
    best?.provider === 'manual' ? t.sourceManual : t.sourceIgdb(igdb?.sampleCount ?? null);
  return (
    <div className={styles.footer}>
      <span>
        {source}
        {igdb && ` · ${t.updated(formatLastPlayed(igdb.fetchedAt))}`}
      </span>
      <span className={styles.footerLinks}>
        <button type="button" className={styles.link} onClick={onCorrect}>
          {t.correct}
        </button>
        {igdb?.url && (
          <button type="button" className={styles.link} onClick={() => open.mutate(igdb.url!)}>
            {t.seeIgdb}
          </button>
        )}
        <button
          type="button"
          className={styles.iconLink}
          aria-label={t.refresh}
          title={t.refresh}
          disabled={refresh.isPending}
          onClick={() => refresh.mutate()}
        >
          <RefreshCw size={14} aria-hidden="true" />
        </button>
      </span>
    </div>
  );
}

const toHours = (m: number | null | undefined) =>
  m ? String(Math.round((m / 60) * 10) / 10).replace('.', ',') : '';

/** Correction (3 fields, in hours) and, when the match was doubtful, the IGDB candidates. */
function CorrectForm({
  gameId,
  view,
  onDone,
}: {
  gameId: string;
  view: TimeToBeatView;
  onDone: () => void;
}) {
  const { setManual, pick } = useTimeToBeatMutations(gameId);
  const manual = view.sources.find((s) => s.provider === 'manual');
  // Only the correction fills the fields; IGDB values are shown as hints, so
  // saving without typing does not freeze them as a correction.
  const igdb = view.sources.find((s) => s.provider === 'igdb');
  const [main, setMain] = useState(toHours(manual?.mainMinutes));
  const [extra, setExtra] = useState(toHours(manual?.extraMinutes));
  const [done, setDone] = useState(toHours(manual?.completionistMinutes));
  const values = [main, extra, done].map(hoursToMinutes);
  const invalid = values.some((v) => v === undefined);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (invalid) return;
    const [m, x, c] = values as (number | null)[];
    setManual.mutate(
      m == null && x == null && c == null ? null : { main: m, extra: x, completionist: c },
      { onSuccess: onDone },
    );
  };

  return (
    <form className={styles.form} onSubmit={submit}>
      {view.candidates.length > 0 && (
        <fieldset className={styles.candidates}>
          <legend className={styles.legend}>{t.candidatesTitle}</legend>
          {view.candidates.map((c) => (
            <button
              key={c.id}
              type="button"
              className={styles.candidate}
              disabled={pick.isPending}
              onClick={() => pick.mutate(c.id, { onSuccess: onDone })}
            >
              {c.name}
              {c.year && <span className={styles.year}> ({c.year})</span>}
            </button>
          ))}
        </fieldset>
      )}
      <p className={styles.hint}>{t.formHint}</p>
      <div className={styles.fields}>
        {(
          [
            ['ttb-main', t.main, main, setMain, igdb?.mainMinutes],
            ['ttb-extra', t.extra, extra, setExtra, igdb?.extraMinutes],
            ['ttb-done', t.completionist, done, setDone, igdb?.completionistMinutes],
          ] as const
        ).map(([id, label, value, set, hint]) => (
          <label key={id} htmlFor={id} className={styles.field}>
            <span>{label}</span>
            <span className={styles.inputWrap}>
              <input
                id={id}
                inputMode="decimal"
                value={value}
                placeholder={toHours(hint)}
                aria-invalid={hoursToMinutes(value) === undefined}
                onChange={(e) => set(e.target.value)}
              />
              <span aria-hidden="true">h</span>
            </span>
          </label>
        ))}
      </div>
      {invalid && (
        <p role="alert" className={styles.error}>
          {t.invalid}
        </p>
      )}
      {setManual.isError && (
        <p role="alert" className={styles.error}>
          {t.saveError}
        </p>
      )}
      <div className={styles.actions}>
        <button type="submit" className={buttons.primary} disabled={invalid || setManual.isPending}>
          {t.save}
        </button>
        {manual && (
          <button
            type="button"
            className={buttons.secondary}
            disabled={setManual.isPending}
            onClick={() => setManual.mutate(null, { onSuccess: onDone })}
          >
            {t.removeCorrection}
          </button>
        )}
        <button type="button" className={buttons.secondary} onClick={onDone}>
          {t.cancel}
        </button>
      </div>
    </form>
  );
}
