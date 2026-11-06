import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { Eye, EyeOff, FileText, FolderPlus, ImageIcon, Play, Star, Trash2, X } from 'lucide-react';
import { artworkSrc, type GameDetail, type GameStatus } from '../../api/client';
import {
  useAddTag,
  useCollectionMembership,
  useCollections,
  useDuplicates,
  useGame,
  useLaunchGame,
  useMergeGames,
  useRemoveTag,
  useSplitSource,
  useUpdateGame,
} from '../../api/hooks';
import { fr } from '../../i18n/fr';
import { formatBytes, formatLastPlayed, formatPlaytimeLong, titleTint } from '../../lib/format';
import { isDeletable } from '../../lib/selection';
import { useGameActions } from '../GameActions/useGameActions';
import { GameTitle } from '../GameTitle/GameTitle';
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
  const { data: game, isError } = useGame(gameId);

  // The game was deleted (or merged away): nothing left to show.
  useEffect(() => {
    if (isError) onClose();
  }, [isError, onClose]);

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
  const actions = useGameActions();
  const banner = artworkSrc(game.hero ?? s.header ?? s.cover);
  const playSource = game.sources.find((src) => src.id === game.playSourceId);
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
        <GameTitle title={s.title} logo={game.logo} className={styles.title} />
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

        <div className={styles.actions}>
          <Link to={`/game/${s.id}`} className={styles.action}>
            <FileText size={18} aria-hidden="true" />
            {fr.actions.openSheet}
          </Link>
          <button
            type="button"
            className={styles.action}
            onClick={() => actions.open({ kind: 'images', game: s })}
          >
            <ImageIcon size={18} aria-hidden="true" />
            {fr.panel.images}
          </button>
          <button
            type="button"
            className={styles.action}
            onClick={() => actions.open({ kind: 'collections', ids: [s.id] })}
          >
            <FolderPlus size={18} aria-hidden="true" />
            {fr.panel.collections}
          </button>
          <button
            type="button"
            className={styles.action}
            onClick={() => update.mutate({ hidden: !s.hidden })}
            disabled={update.isPending}
          >
            {s.hidden ? (
              <Eye size={18} aria-hidden="true" />
            ) : (
              <EyeOff size={18} aria-hidden="true" />
            )}
            {s.hidden ? fr.actions.unhide : fr.actions.hide}
          </button>
          {isDeletable(s) && (
            <button
              type="button"
              className={`${styles.action} ${styles.actionDanger}`}
              onClick={() => actions.open({ kind: 'delete', ids: [s.id], titles: [s.title] })}
            >
              <Trash2 size={18} aria-hidden="true" />
              {fr.actions.delete}
            </button>
          )}
        </div>
        {actions.element}

        <div className={styles.tiles}>
          <StatTile label={fr.panel.totalTime} value={formatPlaytimeLong(total)} />
          {s.achievements && s.achievements.total > 0 ? (
            <StatTile
              label={fr.panel.achievements}
              value={`${s.achievements.done}/${s.achievements.total}`}
            />
          ) : s.goals && s.goals.total > 0 ? (
            <StatTile label={fr.panel.goals} value={`${s.goals.done}/${s.goals.total}`} />
          ) : (
            <StatTile label={fr.panel.achievements} value="—" />
          )}
          <StatTile label={fr.panel.lastSession} value={formatLastPlayed(s.lastPlayed)} />
        </div>

        <DuplicateHint game={game} />

        {total > 0 && <TimeBreakdown game={game} />}

        <SourcesSection game={game} />

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

        {(game.genres.length > 0 || game.modes.length > 0) && (
          <section className={styles.section} aria-labelledby="panel-genres">
            <h3 id="panel-genres" className="section-label">
              {fr.panel.genres}
            </h3>
            <ul className={styles.tags}>
              {[...game.genres, ...game.modes].map((g) => (
                <li key={g} className={styles.tag}>
                  {g}
                </li>
              ))}
            </ul>
          </section>
        )}

        <Tags game={game} />

        <ManualCollections
          gameId={s.id}
          onAdd={() => actions.open({ kind: 'collections', ids: [s.id] })}
        />
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

/** Suggests merging with a game that looks like the same title elsewhere. */
function DuplicateHint({ game }: { game: GameDetail }) {
  const { data: groups } = useDuplicates();
  const merge = useMergeGames();
  const group = groups?.find((g) => g.some((x) => x.id === game.summary.id));
  const others = group?.filter((x) => x.id !== game.summary.id) ?? [];
  if (others.length === 0) return null;
  return (
    <section className={styles.duplicate} aria-labelledby="dup-title">
      <h3 id="dup-title" className="section-label">
        {fr.panel.duplicateTitle}
      </h3>
      {others.map((o) => (
        <p key={o.id}>
          {fr.panel.duplicateBody(o.title, o.sources.map((k) => fr.sources[k]).join(', '))}
        </p>
      ))}
      <button
        type="button"
        className={buttons.secondary}
        disabled={merge.isPending}
        onClick={() =>
          merge.mutate({ targetId: game.summary.id, otherIds: others.map((o) => o.id) })
        }
      >
        {fr.panel.merge}
      </button>
    </section>
  );
}

/** Sources of the game: preferred one for Play, split a merged source. */
function SourcesSection({ game }: { game: GameDetail }) {
  const update = useUpdateGame(game.summary.id);
  const split = useSplitSource();
  if (game.sources.length < 2) return null;
  return (
    <section className={styles.section} aria-labelledby="panel-sources">
      <h3 id="panel-sources" className="section-label">
        {fr.panel.sources}
      </h3>
      <label className={styles.preferred}>
        <span>{fr.panel.preferred}</span>
        <select
          value={game.preferredSourceId ?? ''}
          onChange={(e) => update.mutate({ preferredSourceId: e.target.value })}
        >
          <option value="">{fr.panel.preferredAuto}</option>
          {game.sources.map((src) => (
            <option key={src.id} value={src.id}>
              {fr.sources[src.kind]}
            </option>
          ))}
        </select>
      </label>
      <ul className={styles.sourceList}>
        {game.sources.map((src) => (
          <li key={src.id}>
            <SourceBadge kind={src.kind} />
            <span className="mono">
              {src.playtimeMinutes != null ? formatPlaytimeLong(src.playtimeMinutes) : '—'}
            </span>
            <button
              type="button"
              className={styles.linkButton}
              aria-label={fr.panel.splitLabel(fr.sources[src.kind])}
              disabled={split.isPending}
              onClick={() => split.mutate(src.id)}
            >
              {fr.panel.split}
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}

/** Manual collections holding the game, as chips; changes go through the picker. */
function ManualCollections({ gameId, onAdd }: { gameId: string; onAdd: () => void }) {
  const { data } = useCollections();
  const manual = data?.filter((c) => c.collection.kind === 'manual') ?? [];
  const { data: members } = useCollectionMembership(
    gameId,
    manual.map((c) => c.collection.id),
  );
  const mine = manual.filter((c) => members?.has(c.collection.id));
  return (
    <section className={styles.section} aria-labelledby="panel-collections">
      <h3 id="panel-collections" className="section-label">
        {fr.collections.manualIn}
      </h3>
      <ul className={styles.tags}>
        {mine.map((c) => (
          <li key={c.collection.id}>
            <Link
              to={`/library?view=collection-${c.collection.id}`}
              className={`${styles.tag} ${styles.tagLink}`}
            >
              {c.collection.name}
            </Link>
          </li>
        ))}
        <li>
          <button type="button" className={styles.tagAdd} onClick={onAdd}>
            {fr.panel.addToCollection}
          </button>
        </li>
      </ul>
    </section>
  );
}
