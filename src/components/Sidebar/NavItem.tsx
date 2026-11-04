import type { ReactNode } from 'react';
import { Link, useLocation } from 'react-router-dom';
import { isNavActive } from './navActive';
import styles from './Sidebar.module.css';

interface NavItemProps {
  to: string;
  label: string;
  /** Right-aligned mono counter or arrow. */
  trailing?: ReactNode;
  /** Leading marker (status dot, source chip). */
  leading?: ReactNode;
  size?: 'default' | 'compact';
}

export function NavItem({ to, label, trailing, leading, size = 'default' }: NavItemProps) {
  const location = useLocation();
  const active = isNavActive(location, to);
  return (
    <Link
      to={to}
      className={`${styles.navItem} ${size === 'compact' ? styles.compact : ''} ${active ? styles.active : ''}`}
      aria-current={active ? 'page' : undefined}
    >
      {leading}
      <span className={styles.navLabel}>{label}</span>
      {trailing !== undefined && <span className={styles.counter}>{trailing}</span>}
    </Link>
  );
}
