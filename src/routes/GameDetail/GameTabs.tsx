import { useRef, type KeyboardEvent } from 'react';
import { Link } from 'react-router-dom';
import { fr } from '../../i18n/fr';
import styles from './GameTabs.module.css';

export type SheetTab = 'apercu' | 'succes' | 'goldberg' | 'captures' | 'actus' | 'lancement';

export const SHEET_TABS: SheetTab[] = [
  'apercu',
  'succes',
  'goldberg',
  'captures',
  'actus',
  'lancement',
];

export interface TabItem {
  id: SheetTab;
  label: string;
  /** Small counter after the label ("17/44", "3"). */
  count?: string;
}

/**
 * Sheet sections as links (`aria-current="page"`): Tab moves through them,
 * Left / Right go to the neighbouring tab.
 */
export function GameTabs({
  gameId,
  tabs,
  current,
}: {
  gameId: string;
  tabs: TabItem[];
  current: SheetTab;
}) {
  const refs = useRef<(HTMLAnchorElement | null)[]>([]);
  const onKey = (e: KeyboardEvent, i: number) => {
    const delta = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
    if (!delta) return;
    e.preventDefault();
    const next = refs.current[(i + delta + tabs.length) % tabs.length];
    next?.focus();
    next?.click();
  };
  return (
    <nav className={styles.tabs} aria-label={fr.game.tabsLabel}>
      {tabs.map((tab, i) => (
        <Link
          key={tab.id}
          ref={(el) => {
            refs.current[i] = el;
          }}
          to={tab.id === 'apercu' ? `/game/${gameId}` : `/game/${gameId}/${tab.id}`}
          aria-current={tab.id === current ? 'page' : undefined}
          className={`${styles.tab} ${tab.id === current ? styles.current : ''}`}
          onKeyDown={(e) => onKey(e, i)}
        >
          {tab.label}
          {tab.count && (
            <>
              {' '}
              <span className={styles.count}>{tab.count}</span>
            </>
          )}
        </Link>
      ))}
    </nav>
  );
}
