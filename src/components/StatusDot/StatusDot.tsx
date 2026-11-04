import styles from './StatusDot.module.css';

export type GameStatus = 'todo' | 'playing' | 'done' | 'dropped';

export function StatusDot({ status }: { status: GameStatus }) {
  return <span className={`${styles.dot} ${styles[status]}`} aria-hidden="true" />;
}
