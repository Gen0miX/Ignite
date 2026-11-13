import { useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { Copy, Film, FolderOpen, FolderPlus, ImageIcon, Maximize2 } from 'lucide-react';
import { artworkSrc, type CaptureItem, type GameDetail } from '../../api/client';
import { useCaptureActions, useCaptures, useCapturesScan, useGameMedia } from '../../api/hooks';
import buttons from '../../components/Button/Button.module.css';
import { Skeleton, SkeletonGroup } from '../../components/Skeleton/Skeleton';
import { useToasts } from '../../components/Toaster/toasts';
import { strings } from '../../i18n';
import {
  CAPTURE_SOURCES,
  dayFormat,
  filterCaptures,
  groupCaptures,
  sourceCounts,
  timeFormat,
  type CaptureFilter,
} from '../../lib/captures';
import { formatPlaytimeLong } from '../../lib/format';
import { CaptureViewer } from './CaptureViewer';
import styles from './CapturesTab.module.css';

const c = strings.game.cap;

/** Below this many captures, a card recalls the capture shortcuts. */
const FEW = 4;

/** Shared feedback for the capture actions (grid and viewer). */
export function useCaptureCommands(gameId: string) {
  const actions = useCaptureActions(gameId);
  const show = useToasts((s) => s.show);
  const fail = () => show(c.actionError);
  return {
    actions,
    useAsHero: (id: string) =>
      actions.useAsHero.mutate(id, {
        onSuccess: () => show(c.useAsHeroDone, [gameId], { icon: 'ok' }),
        onError: fail,
      }),
    copy: (id: string) =>
      actions.copy.mutate(id, {
        onSuccess: () => show(c.copied, [], { icon: 'ok' }),
        onError: fail,
      }),
    reveal: (id: string) => actions.reveal.mutate(id, { onError: fail }),
    open: (id: string) => actions.open.mutate(id, { onError: fail }),
  };
}

export function CapturesTab({ game }: { game: GameDetail }) {
  const s = game.summary;
  const view = useCaptures(s.id);
  useCapturesScan(s.id, true);
  const commands = useCaptureCommands(s.id);
  const [filter, setFilter] = useState<CaptureFilter>('all');
  const [viewing, setViewing] = useState<string | null>(null);

  const all = view.data?.items ?? [];
  const counts = sourceCounts(all);
  const items = filterCaptures(all, filter);
  const groups = groupCaptures(items);
  const viewIndex = viewing ? items.findIndex((i) => i.capture.id === viewing) : -1;

  const addFolder = async () => {
    const path = await open({ directory: true, title: c.addFolderTitle });
    if (typeof path === 'string') commands.actions.addFolder.mutate({ path, forGame: s.id });
  };

  const more = all.length < FEW && (
    <div className={styles.more}>
      <p className={styles.moreTitle}>{c.moreTitle}</p>
      <p className={styles.moreBody}>
        {/* The shortcuts are shown as keys inside the sentence. */}
        {c
          .moreBody('\u0000F12\u0000', '\u0000Win+Alt+Impr\u0000')
          .split('\u0000')
          .map((part, i) => (i % 2 === 1 ? <kbd key={i}>{part}</kbd> : part))}
      </p>
    </div>
  );

  return (
    <div className={styles.tab}>
      <div className={styles.toolbar}>
        <div role="group" aria-label={c.filters} className={styles.pills}>
          {(['all', ...CAPTURE_SOURCES] as CaptureFilter[]).map((f) => (
            <button
              key={f}
              type="button"
              className={styles.pill}
              aria-pressed={filter === f}
              data-empty={counts[f] === 0 || undefined}
              onClick={() => setFilter(f)}
            >
              {f === 'all' ? c.all(counts.all) : `${c.source[f]} · ${counts[f]}`}
            </button>
          ))}
        </div>
        <div className={styles.toolbarEnd}>
          {items[0] && (
            <button
              type="button"
              className={buttons.secondary}
              onClick={() => commands.actions.openFolder.mutate(items[0]!.capture.id)}
            >
              <FolderOpen size={16} aria-hidden="true" />
              {c.openFolder}
            </button>
          )}
          <button type="button" className={buttons.secondary} onClick={() => void addFolder()}>
            <FolderPlus size={16} aria-hidden="true" />
            {c.addFolder}
          </button>
        </div>
      </div>

      {view.isPending && (
        <SkeletonGroup label={c.loading}>
          <span className={styles.grid}>
            {[0, 1, 2].map((i) => (
              <Skeleton key={i} aspectRatio="16 / 9" radius="var(--radius-lg)" />
            ))}
          </span>
        </SkeletonGroup>
      )}
      {view.isError && <p className={styles.muted}>{c.error}</p>}

      {view.data && all.length > 0 && items.length === 0 && (
        <p className={styles.muted}>{c.noMatch}</p>
      )}
      {view.data &&
        groups.map((g, gi) => (
          <section key={g.key} className={styles.group} aria-labelledby={`cap-${g.key}`}>
            <div className={styles.groupHead}>
              <h2 id={`cap-${g.key}`} className={styles.groupTitle}>
                {dayFormat.format(g.date)}
              </h2>
              <span className={styles.groupMeta}>
                {g.session
                  ? c.sessionGroup(
                      formatPlaytimeLong(g.session.durationSeconds / 60),
                      g.items.length,
                    )
                  : c.dayGroup(g.items.length)}
              </span>
            </div>
            <ul className={styles.grid}>
              {g.items.map((item) => (
                <li key={item.capture.id} className={styles.cell}>
                  <Tile
                    item={item}
                    title={s.title}
                    onOpen={() => setViewing(item.capture.id)}
                    commands={commands}
                  />
                </li>
              ))}
              {gi === groups.length - 1 && more && <li className={styles.cell}>{more}</li>}
            </ul>
          </section>
        ))}
      {view.data && all.length === 0 && (
        <section className={styles.group} aria-label={c.noneTitle}>
          <div className={styles.grid}>{more}</div>
        </section>
      )}

      <OfficialImages game={game} />
      <p className={styles.note}>{c.readOnly}</p>

      {viewIndex >= 0 && (
        <CaptureViewer
          game={game}
          items={items}
          index={viewIndex}
          onIndex={(i) => setViewing(items[i]?.capture.id ?? null)}
          onClose={() => setViewing(null)}
          commands={commands}
        />
      )}
    </div>
  );
}

function Tile({
  item,
  title,
  onOpen,
  commands,
}: {
  item: CaptureItem;
  title: string;
  onOpen: () => void;
  commands: ReturnType<typeof useCaptureCommands>;
}) {
  const cap = item.capture;
  const [failed, setFailed] = useState(false);
  const src = artworkSrc(cap.thumbPath ?? (cap.kind === 'image' ? cap.path : null));
  const taken = new Date(cap.takenAt);
  const image = cap.kind === 'image';
  return (
    <div className={styles.tile}>
      <button
        type="button"
        className={styles.open}
        aria-label={c.altAt(title, `${dayFormat.format(taken)} ${timeFormat.format(taken)}`)}
        onClick={onOpen}
      >
        {src && !failed ? (
          <img src={src} alt="" loading="lazy" decoding="async" onError={() => setFailed(true)} />
        ) : (
          <span className={styles.placeholder}>
            {image ? (
              <ImageIcon size={28} aria-hidden="true" />
            ) : (
              <Film size={28} aria-hidden="true" />
            )}
            {!image && c.video}
          </span>
        )}
      </button>
      <div className={styles.meta} aria-hidden="true">
        <span className={styles.metaText}>
          {timeFormat.format(taken)}
          {cap.width && cap.height ? ` · ${cap.width}×${cap.height}` : ''}
        </span>
        {!image && <Film size={14} />}
        <span className={styles.badge} data-source={cap.source}>
          {c.badge[cap.source]}
        </span>
      </div>
      <div className={styles.actions}>
        <TileAction label={c.enlarge} onClick={onOpen}>
          <Maximize2 size={16} />
        </TileAction>
        {image && (
          <TileAction label={c.useAsHero} onClick={() => commands.useAsHero(cap.id)}>
            <ImageIcon size={16} />
          </TileAction>
        )}
        {image && (
          <TileAction label={c.copy} onClick={() => commands.copy(cap.id)}>
            <Copy size={16} />
          </TileAction>
        )}
        <TileAction label={c.reveal} onClick={() => commands.reveal(cap.id)}>
          <FolderOpen size={16} />
        </TileAction>
      </div>
    </div>
  );
}

function TileAction({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      className={styles.action}
      aria-label={label}
      title={label}
      onClick={onClick}
    >
      <span aria-hidden="true">{children}</span>
    </button>
  );
}

/** Hero, header and store screenshots: any of them can become the background. */
function OfficialImages({ game }: { game: GameDetail }) {
  const s = game.summary;
  const media = useGameMedia(s.id);
  const show = useToasts((t) => t.show);
  const store = useCaptureActions(s.id).useStoreImage;
  const current = game.hero ?? s.header;
  const local: { path: string; label: string }[] = [];
  if (game.hero) local.push({ path: game.hero, label: c.officialHero });
  if (s.header) local.push({ path: s.header, label: c.officialHeader });
  const shots = media.data?.media.screenshots ?? [];
  if (local.length === 0 && shots.length === 0) return null;
  return (
    <section className={styles.group} aria-labelledby="cap-official">
      <div className={styles.groupHead}>
        <h2 id="cap-official" className={styles.groupTitle}>
          {c.official}
        </h2>
        <span className={styles.groupMeta}>{c.officialHint}</span>
      </div>
      <ul className={`${styles.grid} ${styles.gridSmall}`}>
        {local.map((x) => (
          <li key={x.path} className={styles.cell}>
            <div className={styles.tile}>
              <img className={styles.still} src={artworkSrc(x.path)} alt={x.label} loading="lazy" />
              {x.path === current && <span className={styles.current}>{c.currentHero}</span>}
            </div>
          </li>
        ))}
        {shots.map((shot, i) => (
          <li key={shot.full} className={styles.cell}>
            <div className={styles.tile}>
              <img
                className={styles.still}
                src={shot.thumb}
                alt={c.officialShot(i + 1)}
                loading="lazy"
              />
              <div className={styles.actions}>
                <TileAction
                  label={c.useAsHero}
                  onClick={() =>
                    store.mutate(shot.full, {
                      onSuccess: () => show(c.useAsHeroDone, [s.id], { icon: 'ok' }),
                      onError: () => show(c.actionError),
                    })
                  }
                >
                  <ImageIcon size={16} />
                </TileAction>
              </div>
            </div>
          </li>
        ))}
      </ul>
    </section>
  );
}
