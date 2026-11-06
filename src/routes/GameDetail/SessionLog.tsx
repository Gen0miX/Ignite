import { useState, type FormEvent } from 'react';
import { Pencil, Trash2 } from 'lucide-react';
import type { GameDetail, PlaySession, SessionOrigin } from '../../api/client';
import { useDeleteSession, useSaveSession } from '../../api/hooks';
import buttons from '../../components/Button/Button.module.css';
import { fr } from '../../i18n/fr';
import { formatPlaytimeLong, toLocalInput } from '../../lib/format';
import styles from './GameDetailPage.module.css';
import { FlameRating } from '../../components/FlameRating/FlameRating';

const t = fr.game;
const dateFmt = new Intl.DateTimeFormat('fr-FR', {
  day: 'numeric',
  month: 'short',
  year: 'numeric',
});
const timeFmt = new Intl.DateTimeFormat('fr-FR', { hour: '2-digit', minute: '2-digit' });

type Draft = {
  id: string | null;
  origin: SessionOrigin;
  start: string;
  minutes: string;
  note: string;
};

function draftFor(session: PlaySession | null, origin: SessionOrigin): Draft {
  if (session) {
    return {
      id: session.id,
      origin: session.origin,
      start: session.startedAt ? toLocalInput(new Date(session.startedAt)) : '',
      minutes:
        session.origin === 'estimate'
          ? String(Math.round(session.durationSeconds / 360) / 10)
          : String(Math.round(session.durationSeconds / 60)),
      note: session.note ?? '',
    };
  }
  const start = new Date(Date.now() - 60 * 60 * 1000);
  return {
    id: null,
    origin,
    start: toLocalInput(start),
    minutes: origin === 'estimate' ? '10' : '60',
    note: '',
  };
}

function SessionForm({
  gameId,
  draft,
  onDone,
}: {
  gameId: string;
  draft: Draft;
  onDone: () => void;
}) {
  const [d, setD] = useState(draft);
  const save = useSaveSession();
  const estimate = d.origin === 'estimate';

  const submit = (e: FormEvent) => {
    e.preventDefault();
    const value = Number(d.minutes.replace(',', '.'));
    const durationMinutes = Math.round(estimate ? value * 60 : value);
    save.mutate(
      {
        id: d.id,
        gameId,
        origin: d.origin,
        startedAt: estimate ? null : new Date(d.start).toISOString(),
        durationMinutes: Number.isFinite(durationMinutes) ? Math.max(0, durationMinutes) : 0,
        note: d.note,
      },
      { onSuccess: onDone },
    );
  };

  return (
    <form className={styles.sessionForm} onSubmit={submit}>
      {!estimate && (
        <label className={styles.formField}>
          <span>{t.fieldStart}</span>
          <input
            type="datetime-local"
            required
            value={d.start}
            max={toLocalInput(new Date())}
            onChange={(e) => setD({ ...d, start: e.target.value })}
          />
        </label>
      )}
      <label className={styles.formField}>
        <span>{estimate ? t.fieldEstimate : t.fieldDuration}</span>
        <input
          type="number"
          required
          min={estimate ? 0.1 : 1}
          step={estimate ? 0.5 : 1}
          inputMode="decimal"
          value={d.minutes}
          onChange={(e) => setD({ ...d, minutes: e.target.value })}
        />
      </label>
      <label className={`${styles.formField} ${styles.formWide}`}>
        <span>{t.fieldNote}</span>
        <input
          type="text"
          maxLength={500}
          value={d.note}
          onChange={(e) => setD({ ...d, note: e.target.value })}
        />
      </label>
      <div className={styles.formActions}>
        <button type="submit" className={buttons.primary} disabled={save.isPending}>
          {t.save}
        </button>
        <button type="button" className={buttons.secondary} onClick={onDone}>
          {t.cancel}
        </button>
      </div>
      {save.isError && (
        <p role="alert" className={styles.error}>
          {t.sessionError}
        </p>
      )}
    </form>
  );
}

export function SessionLog({
  game,
  sessions,
  steamCounted,
}: {
  game: GameDetail;
  sessions: PlaySession[];
  steamCounted: boolean;
}) {
  const [editing, setEditing] = useState<Draft | null>(null);
  const remove = useDeleteSession();
  const gameId = game.summary.id;

  return (
    <section className={styles.section} aria-labelledby="sheet-log">
      <div className={styles.sectionHeader}>
        <h2 id="sheet-log" className="section-label">
          {t.log}
        </h2>
        <div className={styles.sectionActions}>
          <button
            type="button"
            className={buttons.secondary}
            onClick={() => setEditing(draftFor(null, 'manual'))}
          >
            {t.addSession}
          </button>
          <button
            type="button"
            className={buttons.secondary}
            onClick={() => setEditing(draftFor(null, 'estimate'))}
          >
            {t.addEstimate}
          </button>
        </div>
      </div>
      {steamCounted && <p className={styles.muted}>{t.steamCounterNote}</p>}
      {editing && !editing.id && (
        <SessionForm gameId={gameId} draft={editing} onDone={() => setEditing(null)} />
      )}

      {sessions.length === 0 ? (
        <p className={styles.muted}>{t.logEmpty}</p>
      ) : (
        <div className={styles.tableWrap}>
          <table className={styles.table}>
            <thead>
              <tr>
                <th scope="col">{t.colDate}</th>
                <th scope="col">{t.colTime}</th>
                <th scope="col">{t.colDuration}</th>
                <th scope="col">{t.colRating}</th>
                <th scope="col">{t.colOrigin}</th>
                <th scope="col">{t.colNote}</th>
                <th scope="col">
                  <span className="visually-hidden">Actions</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {sessions.map((s) =>
                editing?.id === s.id ? (
                  <tr key={s.id}>
                    <td colSpan={7}>
                      <SessionForm
                        gameId={gameId}
                        draft={editing}
                        onDone={() => setEditing(null)}
                      />
                    </td>
                  </tr>
                ) : (
                  <tr key={s.id}>
                    <td>{s.startedAt ? dateFmt.format(new Date(s.startedAt)) : '—'}</td>
                    <td className="mono">
                      {s.startedAt && s.endedAt
                        ? `${timeFmt.format(new Date(s.startedAt))}–${timeFmt.format(new Date(s.endedAt))}`
                        : '—'}
                    </td>
                    <td className="mono">
                      {formatPlaytimeLong(s.durationSeconds / 60)}
                      {s.idleSeconds >= 60 && (
                        <span
                          className={styles.subtle}
                          title={t.idleRemoved(formatPlaytimeLong(s.idleSeconds / 60))}
                        >
                          {' '}
                          (−{formatPlaytimeLong(s.idleSeconds / 60)})
                        </span>
                      )}
                    </td>
                    <td>
                      <FlameRating value={s.rating} />
                    </td>
                    <td>
                      <span className={`${styles.origin} ${styles[`origin_${s.origin}`]}`}>
                        {t.origin[s.origin]}
                      </span>
                    </td>
                    <td className={styles.noteCell}>{s.note ?? ''}</td>
                    <td className={styles.rowActions}>
                      <button
                        type="button"
                        className={styles.iconAction}
                        aria-label={t.edit}
                        title={t.edit}
                        onClick={() => setEditing(draftFor(s, s.origin))}
                      >
                        <Pencil size={15} aria-hidden="true" />
                      </button>
                      <button
                        type="button"
                        className={styles.iconAction}
                        aria-label={t.delete}
                        title={t.delete}
                        onClick={() => {
                          if (window.confirm(t.confirmDelete)) remove.mutate(s.id);
                        }}
                      >
                        <Trash2 size={15} aria-hidden="true" />
                      </button>
                    </td>
                  </tr>
                ),
              )}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
