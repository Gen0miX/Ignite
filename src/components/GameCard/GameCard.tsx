import { forwardRef, type KeyboardEvent, type MouseEvent } from 'react';
import { Check, EyeOff, MoreHorizontal, Play } from 'lucide-react';
import type { GameSummary } from '../../api/client';
import { fr } from '../../i18n/fr';
import { formatPlaytimeShort } from '../../lib/format';
import { useCardDrag } from '../../lib/gameDrag';
import { progressLabel } from '../../lib/sessions';
import { SourceBadge } from '../SourceBadge/SourceBadge';
import { Cover } from './Cover';
import styles from './GameCard.module.css';

interface GameCardProps {
  game: GameSummary;
  selected: boolean;
  /** Ticked in the multi-selection. */
  checked: boolean;
  /** At least one game is ticked: a click toggles instead of opening the panel. */
  selecting: boolean;
  playing?: boolean;
  tabIndex: number;
  onSelect: () => void;
  onToggle: (shift: boolean) => void;
  onLaunch: () => void;
  onMenu: (at: { x: number; y: number }) => void;
  /** Games dragged with this card (the selection when the card is part of it). */
  dragIds: () => string[];
  onKeyDown: (e: KeyboardEvent<HTMLDivElement>) => void;
}

/**
 * Cover card: click selects (or toggles in selection mode), Ctrl/Shift+click
 * multi-selects, double-click / Enter launches, right-click or "⋯" opens the
 * actions, drag onto a sidebar collection adds to it.
 */
export const GameCard = forwardRef<HTMLDivElement, GameCardProps>(function GameCard(
  {
    game,
    selected,
    checked,
    selecting,
    playing = false,
    tabIndex,
    onSelect,
    onToggle,
    onLaunch,
    onMenu,
    dragIds,
    onKeyDown,
  },
  ref,
) {
  const drag = useCardDrag(dragIds);
  const meta = [formatPlaytimeShort(game.playtimeMinutes)];
  const progress = progressLabel(game.achievements, game.goals);
  if (progress) meta.push(progress);
  else if (game.installed) meta.push(fr.library.installedBadge);

  const onClick = (e: MouseEvent) => {
    if (drag.dragged()) return;
    if (e.ctrlKey || e.metaKey || e.shiftKey || selecting) onToggle(e.shiftKey);
    else onSelect();
  };

  const openMenuFromButton = (e: MouseEvent<HTMLButtonElement>) => {
    e.stopPropagation();
    const r = e.currentTarget.getBoundingClientRect();
    onMenu({ x: r.left, y: r.bottom + 4 });
  };

  return (
    <div
      ref={ref}
      role="gridcell"
      aria-selected={selected}
      tabIndex={tabIndex}
      className={`${styles.card} ${selected ? styles.selected : ''} ${checked ? styles.checked : ''} ${selecting ? styles.selecting : ''}`}
      onClick={onClick}
      onDoubleClick={() => !selecting && onLaunch()}
      onFocus={() => !selecting && onSelect()}
      onContextMenu={(e) => {
        e.preventDefault();
        onMenu({ x: e.clientX, y: e.clientY });
      }}
      onKeyDown={(e) => {
        if ((e.key === 'F10' && e.shiftKey) || e.key === 'ContextMenu') {
          e.preventDefault();
          const r = e.currentTarget.getBoundingClientRect();
          onMenu({ x: r.left + 12, y: r.top + 12 });
          return;
        }
        if (e.key === ' ') {
          e.preventDefault();
          onToggle(e.shiftKey);
          return;
        }
        onKeyDown(e);
      }}
      aria-label={game.title}
      data-game-id={game.id}
      {...drag.handlers}
    >
      <div className={styles.coverBox}>
        <Cover title={game.title} cover={game.cover} header={game.header} />
        {playing && <span className={styles.playing}>{fr.tracking.inGame}</span>}
        {game.hidden && (
          <span className={styles.hiddenMark} title={fr.library.hiddenTitle}>
            <EyeOff size={14} aria-hidden="true" />
          </span>
        )}
        {game.sources[0] && (
          <span className={styles.badge}>
            <SourceBadge kind={game.sources[0]} overlay />
          </span>
        )}
        <button
          type="button"
          role="checkbox"
          aria-checked={checked}
          tabIndex={-1}
          className={styles.check}
          aria-label={fr.actions.select(game.title)}
          onPointerDown={(e) => e.stopPropagation()}
          onClick={(e) => {
            e.stopPropagation();
            onToggle(e.shiftKey);
          }}
        >
          {checked && <Check size={16} strokeWidth={3} aria-hidden="true" />}
        </button>
        <button
          type="button"
          className={styles.more}
          tabIndex={-1}
          aria-label={fr.actions.menu(game.title)}
          aria-haspopup="menu"
          onPointerDown={(e) => e.stopPropagation()}
          onClick={openMenuFromButton}
        >
          <MoreHorizontal size={18} aria-hidden="true" />
        </button>
        {!selecting && (
          <button
            type="button"
            className={styles.play}
            tabIndex={-1}
            aria-label={`${fr.library.play} : ${game.title}`}
            onPointerDown={(e) => e.stopPropagation()}
            onClick={(e) => {
              e.stopPropagation();
              onLaunch();
            }}
          >
            <Play size={20} fill="currentColor" aria-hidden="true" />
          </button>
        )}
      </div>
      <div className={styles.title}>{game.title}</div>
      <div className={styles.meta}>{meta.join(' · ')}</div>
    </div>
  );
});
