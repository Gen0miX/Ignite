import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { Eye, EyeOff, Play, Star, X } from 'lucide-react';
import { artworkSrc, type GameDetail, type GameStatus } from '../../api/client';
import { useAddTag, useGame, useLaunchGame, useRemoveTag, useUpdateGame } from '../../api/hooks';
import { fr } from '../../i18n/fr';
import { formatBytes, formatLastPlayed, formatPlaytimeLong, titleTint } from '../../lib/format';
import { SegmentedControl } from '../SegmentedControl/SegmentedControl';
import { SourceBadge } from '../SourceBadge/SourceBadge';
import buttons from '../Button/Button.module.css';
import styles from './GamePanel.module.css';

const STATUS_ORDER: GameStatus[] = ['todo', 'playing', 'done', 'dropped'];

interface GamePanelProps {
  gameId: string;
  onClose: () => void;
}

export function GamePanel({ gameId, onClose }: GamePanelProps) {
  const { data: game } = useGame(gameId);

  // Escape closes the panel (also the drawer on narrow windows).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (
        e.key === 'Escape' &&
        !(e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLInputElement)
      ) {
        onClose();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  return (
    <aside className={styles.panel} aria-label={fr.panel.label}>
      <button type="button" className={styles.close} onClick={onClose} aria-label={fr.panel.close}>
        <X size={18} aria-hidden="true" />
      </button>
      {game ? (
        <PanelContent key={game.summary.id} game={game} />
      ) : (
        <div className={styles.skeleton} />
      )}
    </aside>
  );
}

function PanelContent({ game }: { game: GameDetail }) {
  const s = game.summary;
  const update = useUpdateGame(s.id);
  const launch = useLaunchGame();
  const banner = artworkSrc(game.hero ?? s.header ?? s.cover);
  const playSource = game.sources.find((src) => src.launchUri);
  const total = s.playtimeMinutes;

  const launchError =
    launch.error && 'message' in launch.error && /not installed/i.test(launch.error.message)
      ? fr.panel.steamMissing
      : launch.error
        ? fr.panel.launchError
        : null;

  return (
    <>
      <div
        className={styles.banner}
        style={banner ? undefined : { background: titleTint(s.title) }}
      >
        {banner && <img src={banner} alt="" />}
        <div className={styles.bannerShade} />
        <h2 className={styles.title}>{s.title}</h2>
      </div>

      <div className={styles.body}>
        {s.hidden && <p className={styles.notice}>{fr.panel.hiddenNotice}</p>}
        <div className={styles.row}>
          {s.sources.map((k) => (
            <SourceBadge key={k} kind={k} />
          ))}
          <button
            type="button"
            className={`${styles.iconButton} ${s.favorite ? styles.favorite : ''}`}
            aria-pressed={s.favorite}
            aria-label={s.favorite ? fr.panel.favoriteRemove : fr.panel.favoriteAdd}
            title={s.favorite ? fr.panel.favoriteRemove : fr.panel.favoriteAdd}
            onClick={() => update.mutate({ favorite: !s.favorite })}
          >
            <Star size={18} fill={s.favorite ? 'currentColor' : 'none'} aria-hidden="true" />
          </button>
        </div>

        <button
          type="button"
          className={`${buttons.primary} ${styles.play}`}
          disabled={!playSource || launch.isPending}
          onClick={() => launch.mutate(s.id)}
        >
          <Play size={18} fill="currentColor" aria-hidden="true" />
          {launch.isPending
            ? fr.panel.launching
            : playSource
              ? fr.panel.playVia(fr.sources[playSource.kind])
              : fr.panel.play}
        </button>
        {launchError && (
          <p role="alert" className={styles.error}>
            {launchError}
          </p>
        )}

        <div className={styles.tiles}>
          <StatTile label={fr.panel.totalTime} value={formatPlaytimeLong(total)} />
          <StatTile label={fr.panel.achievements} value="—" />
          <StatTile label={fr.panel.lastSession} value={formatLastPlayed(s.lastPlayed)} />
        </div>

        {total > 0 && <TimeBreakdown game={game} />}

        <section className={styles.section}>
          <h3 className="section-label">{fr.panel.status}</h3>
          <SegmentedControl
            label={fr.panel.status}
            options={STATUS_ORDER.map((v) => ({ value: v, label: fr.statusSingular[v] }))}
            value={s.status}
            onChange={(status) => update.mutate({ status })}
          />
        </section>

        <Notes game={game} onSave={(notes) => update.mutate({ notes })} saving={update.isPending} />

        <section className={styles.section}>
          <h3 className="section-label">{fr.panel.details}</h3>
          <dl className={styles.details}>
            {game.sources.map((src) => (
              <div key={src.id} className={styles.detailGroup}>
                <dt>{fr.panel.installPath}</dt>
                <dd className="mono">
                  {src.installed ? (src.installPath ?? '—') : fr.panel.notInstalled}
                </dd>
                {src.installed && src.sizeBytes ? (
                  <>
                    <dt>{fr.panel.size}</dt>
                    <dd className="mono">{formatBytes(src.sizeBytes)}</dd>
                  </>
                ) : null}
              </div>
            ))}
          </dl>
        </section>

        <Tags game={game} />

        <div className={styles.footerActions}>
          <Link to={`/game/${s.id}`} className={buttons.secondary}>
            {fr.panel.openSheet}
          </Link>
          <button
            type="button"
            className={buttons.secondary}
            onClick={() => update.mutate({ hidden: !s.hidden })}
            disabled={update.isPending}
          >
            {s.hidden ? (
              <Eye size={16} aria-hidden="true" />
            ) : (
              <EyeOff size={16} aria-hidden="true" />
            )}
            {s.hidden ? fr.panel.unhide : fr.panel.hide}
          </button>
        </div>
      </div>
    </>
  );
}

function StatTile({ label, value }: { label: string; value: string }) {
  return (
    <div className={styles.tile}>
      <span className={styles.tileValue}>{value}</span>
      <span className={styles.tileLabel}>{label}</span>
    </div>
  );
}

function TimeBreakdown({ game }: { game: GameDetail }) {
  const parts = game.timeBySource.filter((p) => p.minutes > 0);
  const total = parts.reduce((sum, p) => sum + p.minutes, 0);
  if (total === 0) return null;
  return (
    <section className={styles.section}>
      <h3 className="section-label">{fr.panel.timeOrigin}</h3>
      <div className={styles.bar} aria-hidden="true">
        {parts.map((p, i) => (
          <span
            key={p.sourceId ?? `other-${i}`}
            style={{
              width: `${(p.minutes / total) * 100}%`,
              background: p.kind ? `var(--source-${p.kind})` : 'var(--color-muted)',
            }}
          />
        ))}
      </div>
      <ul className={styles.legend}>
        {parts.map((p, i) => (
          <li key={p.sourceId ?? `other-${i}`}>
            <span
              className={styles.legendDot}
              style={{ background: p.kind ? `var(--source-${p.kind})` : 'var(--color-muted)' }}
            />
            <span>
              {p.kind ? fr.sources[p.kind] : fr.panel.otherSessions}
              {p.fromPlatform && (
                <span className={styles.muted}> · {fr.panel.platformCounter}</span>
              )}
            </span>
            <span className="mono">{formatPlaytimeLong(p.minutes)}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}

function Notes({
  game,
  onSave,
  saving,
}: {
  game: GameDetail;
  onSave: (notes: string) => void;
  saving: boolean;
}) {
  const [draft, setDraft] = useState(game.notes ?? '');
  const dirty = draft.trim() !== (game.notes ?? '').trim();
  return (
    <section className={styles.section}>
      <label htmlFor="game-notes" className="section-label">
        {fr.panel.notes}
      </label>
      <textarea
        id="game-notes"
        className={styles.notes}
        value={draft}
        placeholder={fr.panel.notesPlaceholder}
        rows={3}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={() => dirty && onSave(draft)}
      />
      {saving && <span className={styles.muted}>{fr.panel.saved}</span>}
    </section>
  );
}

function Tags({ game }: { game: GameDetail }) {
  const add = useAddTag(game.summary.id);
  const remove = useRemoveTag(game.summary.id);
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState('');

  const submit = () => {
    const value = name.trim();
    if (value) add.mutate(value);
    setName('');
    setEditing(false);
  };

  return (
    <section className={styles.section}>
      <h3 className="section-label">{fr.panel.tags}</h3>
      <ul className={styles.tags}>
        {game.tags.map((t) => (
          <li key={t.id} className={styles.tag}>
            {t.name}
            <button
              type="button"
              className={styles.tagRemove}
              aria-label={fr.panel.removeTag(t.name)}
              onClick={() => remove.mutate(t.id)}
            >
              <X size={12} aria-hidden="true" />
            </button>
          </li>
        ))}
        <li>
          {editing ? (
            <input
              autoFocus
              className={styles.tagInput}
              aria-label={fr.panel.addTagLabel}
              value={name}
              maxLength={40}
              onChange={(e) => setName(e.target.value)}
              onBlur={submit}
              onKeyDown={(e) => {
                if (e.key === 'Enter') submit();
                if (e.key === 'Escape') {
                  setName('');
                  setEditing(false);
                }
              }}
            />
          ) : (
            <button type="button" className={styles.tagAdd} onClick={() => setEditing(true)}>
              {fr.panel.addTag}
            </button>
          )}
        </li>
      </ul>
    </section>
  );
}
