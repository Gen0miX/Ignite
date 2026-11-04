import { useRef, type KeyboardEvent } from 'react';
import type { GameSummary } from '../../api/client';
import { fr } from '../../i18n/fr';
import { GameCard } from '../GameCard/GameCard';
import { countColumns, nextIndex } from './gridNavigation';
import styles from './GameGrid.module.css';

interface GameGridProps {
  games: GameSummary[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onLaunch: (id: string) => void;
}

/** Cover grid with roving focus: arrows move, Enter launches, Home/End jump. */
export function GameGrid({ games, selectedId, onSelect, onLaunch }: GameGridProps) {
  const refs = useRef<(HTMLDivElement | null)[]>([]);
  const selectedIndex = games.findIndex((g) => g.id === selectedId);
  const focusIndex = selectedIndex >= 0 ? selectedIndex : 0;

  const handleKey = (index: number) => (e: KeyboardEvent<HTMLDivElement>) => {
    const game = games[index];
    if (!game) return;
    if (e.key === 'Enter') {
      e.preventDefault();
      onLaunch(game.id);
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
    <div role="grid" aria-label={fr.library.gridLabel} className={styles.grid}>
      <div role="row" className={styles.row}>
        {games.map((game, i) => (
          <GameCard
            key={game.id}
            ref={(el) => {
              refs.current[i] = el;
            }}
            game={game}
            selected={game.id === selectedId}
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
