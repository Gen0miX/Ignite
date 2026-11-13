import { useRef, type CSSProperties, type KeyboardEvent } from 'react';
import type { GameSummary } from '../../api/client';
import { strings } from '../../i18n';
import { GameCard } from '../GameCard/GameCard';
import { usePrefs } from '../../lib/prefs';
import { useGridFlip } from '../../lib/useGridFlip';
import { countColumns, nextIndex } from './gridNavigation';
import styles from './GameGrid.module.css';

interface GameGridProps {
  games: GameSummary[];
  selectedId: string | null;
  playingIds?: Set<string>;
  /** Multi-selection (ticked games). */
  checkedIds: ReadonlySet<string>;
  onSelect: (id: string) => void;
  onToggle: (id: string, shift: boolean) => void;
  onLaunch: (id: string) => void;
  onMenu: (game: GameSummary, at: { x: number; y: number }) => void;
}

/** Cover grid with roving focus: arrows move, Enter launches, Home/End jump. */
export function GameGrid({
  games,
  selectedId,
  playingIds,
  checkedIds,
  onSelect,
  onToggle,
  onLaunch,
  onMenu,
}: GameGridProps) {
  const selecting = checkedIds.size > 0;
  const refs = useRef<(HTMLDivElement | null)[]>([]);
  const coverSize = usePrefs((p) => p.coverSize);
  const rowRef = useGridFlip<HTMLDivElement>(games, coverSize);
  const selectedIndex = games.findIndex((g) => g.id === selectedId);
  const focusIndex = selectedIndex >= 0 ? selectedIndex : 0;

  const handleKey = (index: number) => (e: KeyboardEvent<HTMLDivElement>) => {
    const game = games[index];
    if (!game) return;
    if (e.key === 'Enter') {
      e.preventDefault();
      if (selecting) onToggle(game.id, false);
      else onLaunch(game.id);
      return;
    }
    const items = refs.current.slice(0, games.length).filter((el): el is HTMLDivElement => !!el);
    const target = nextIndex(e.key, index, games.length, countColumns(items));
    if (target !== index && target >= 0) {
      e.preventDefault();
      refs.current[target]?.focus();
      refs.current[target]?.scrollIntoView?.({ block: 'nearest' });
    }
  };

  return (
    <div role="grid" aria-label={strings.library.gridLabel} className={styles.grid}>
      <div
        role="row"
        className={styles.row}
        ref={rowRef}
        style={{ '--cover-min': `${coverSize}px` } as CSSProperties}
      >
        {games.map((game, i) => (
          <GameCard
            key={game.id}
            ref={(el) => {
              refs.current[i] = el;
            }}
            game={game}
            selected={game.id === selectedId}
            checked={checkedIds.has(game.id)}
            selecting={selecting}
            onToggle={(shift) => onToggle(game.id, shift)}
            onMenu={(at) => onMenu(game, at)}
            dragIds={() => (checkedIds.has(game.id) ? [...checkedIds] : [game.id])}
            playing={playingIds?.has(game.id) ?? false}
            tabIndex={i === focusIndex ? 0 : -1}
            onSelect={() => onSelect(game.id)}
            onLaunch={() => onLaunch(game.id)}
            onKeyDown={handleKey(i)}
          />
        ))}
      </div>
    </div>
  );
}
