import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import styles from './Menu.module.css';

export interface MenuItem {
  label: string;
  icon?: ReactNode;
  onSelect: () => void;
  danger?: boolean;
  disabled?: boolean;
  /** Draws a separator above the item. */
  separator?: boolean;
}

interface MenuProps {
  label: string;
  /** Viewport point where the menu opens (cursor or button corner). */
  at: { x: number; y: number };
  items: MenuItem[];
  onClose: () => void;
}

/** Popup menu (role="menu"): arrows move, Enter selects, Escape / Tab / outside click close. */
export function Menu({ label, at, items, onClose }: MenuProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState(at);

  // Keep the menu inside the window.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    setPos({
      x: Math.max(8, Math.min(at.x, window.innerWidth - r.width - 8)),
      y: Math.max(8, Math.min(at.y, window.innerHeight - r.height - 8)),
    });
  }, [at]);

  useEffect(() => {
    const restore = document.activeElement as HTMLElement | null;
    ref.current?.querySelector<HTMLElement>('[role="menuitem"]:not(:disabled)')?.focus();
    const onDown = (e: PointerEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    window.addEventListener('pointerdown', onDown, true);
    return () => {
      window.removeEventListener('pointerdown', onDown, true);
      restore?.focus?.({ preventScroll: true });
    };
  }, [onClose]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    const all = [
      ...(ref.current?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)') ??
        []),
    ];
    const i = all.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const next = (i + (e.key === 'ArrowDown' ? 1 : -1) + all.length) % all.length;
      all[next]?.focus();
    } else if (e.key === 'Home' || e.key === 'End') {
      e.preventDefault();
      all[e.key === 'Home' ? 0 : all.length - 1]?.focus();
    } else if (e.key === 'Escape' || e.key === 'Tab') {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    }
  };

  return createPortal(
    <div
      ref={ref}
      role="menu"
      aria-label={label}
      className={styles.menu}
      style={{ left: pos.x, top: pos.y }}
      onKeyDown={onKeyDown}
      onContextMenu={(e) => e.preventDefault()}
    >
      {items.map((item) => (
        <div key={item.label}>
          {item.separator && <hr className={styles.separator} />}
          <button
            type="button"
            role="menuitem"
            className={`${styles.item} ${item.danger ? styles.danger : ''}`}
            disabled={item.disabled}
            onClick={() => {
              onClose();
              item.onSelect();
            }}
          >
            <span className={styles.icon} aria-hidden="true">
              {item.icon}
            </span>
            {item.label}
          </button>
        </div>
      ))}
    </div>,
    document.body,
  );
}
