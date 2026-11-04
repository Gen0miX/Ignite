import type { ReactNode } from 'react';
import styles from './EmptyState.module.css';

interface EmptyStateProps {
  title?: string;
  children: ReactNode;
  action?: ReactNode;
}

/** Short message plus an optional action (DESIGN.md › États et retours). */
export function EmptyState({ title, children, action }: EmptyStateProps) {
  return (
    <div className={styles.empty}>
      {title && <h2 className={styles.title}>{title}</h2>}
      <p className={styles.body}>{children}</p>
      {action}
    </div>
  );
}
