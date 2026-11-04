import type { ReactNode } from 'react';
import styles from './PageHeader.module.css';

interface PageHeaderProps {
  title: string;
  /** Mono meta text next to the title, e.g. "206 jeux · 7 sources". */
  meta?: string;
  actions?: ReactNode;
}

export function PageHeader({ title, meta, actions }: PageHeaderProps) {
  return (
    <header className={styles.header}>
      <h1 className={styles.title}>{title}</h1>
      {meta && <span className={styles.meta}>{meta}</span>}
      {actions && <div className={styles.actions}>{actions}</div>}
    </header>
  );
}
