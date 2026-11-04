import { forwardRef, type KeyboardEvent } from 'react';
import { Play } from 'lucide-react';
import type { GameSummary } from '../../api/client';
import { fr } from '../../i18n/fr';
import { formatPlaytimeShort } from '../../lib/format';
import { progressLabel } from '../../lib/sessions';
import { SourceBadge } from '../SourceBadge/SourceBadge';
import { Cover } from './Cover';
import styles from './GameCard.module.css';

interface GameCardProps {
  game: GameSummary;
  selected: boolean;
  playing?: boolean;
  tabIndex: number;
  onSelect: () => void;
  onLaunch: () => void;
  onKeyDown: (e: KeyboardEvent<HTMLDivElement>) => void;
}

/** Cover card: click selects, double-click / Enter launches. */
export const GameCard = forwardRef<HTMLDivElement, GameCardProps>(function GameCard(
  { game, selected, playing = false, tabIndex, onSelect, onLaunch, onKeyDown },
  ref,
) {
  const meta = [formatPlaytimeShort(game.playtimeMinutes)];
  const progress = progressLabel(game.achievements, game.goals);
  if (progress) meta.push(progress);
  else if (game.installed) meta.push(fr.library.installedBadge);

  return (
    <div
      ref={ref}
      role="gridcell"
      aria-selected={selected}
      tabIndex={tabIndex}
      className={`${styles.card} ${selected ? styles.selected : ''}`}
      onClick={onSelect}
      onDoubleClick={onLaunch}
      onFocus={onSelect}
      onKeyDown={onKeyDown}
      aria-label={game.title}
      data-game-id={game.id}
    >
      <div className={styles.coverBox}>
        <Cover title={game.title} cover={game.cover} header={game.header} />
        {playing && <span className={styles.playing}>{fr.tracking.inGame}</span>}
        {game.sources[0] && (
          <span className={styles.badge}>
            <SourceBadge kind={game.sources[0]} overlay />
          </span>
        )}
        <button
          type="button"
          className={styles.play}
          tabIndex={-1}
          aria-label={`${fr.library.play} : ${game.title}`}
          onClick={(e) => {
            e.stopPropagation();
            onLaunch();
          }}
        >
          <Play size={20} fill="currentColor" aria-hidden="true" />
        </button>
      </div>
      <div className={styles.title}>{game.title}</div>
      <div className={styles.meta}>{meta.join(' · ')}</div>
    </div>
  );
});
