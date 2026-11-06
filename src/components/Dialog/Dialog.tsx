import { useEffect, useId, useRef, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { X } from 'lucide-react';
import styles from './Dialog.module.css';

interface DialogProps {
  title: string;
  closeLabel: string;
  onClose: () => void;
  children: ReactNode;
  /** `alertdialog` for confirmations. */
  role?: 'dialog' | 'alertdialog';
  width?: number;
}

/** Small modal: Escape and the backdrop close it, focus goes back to the opener. */
export function Dialog({
  title,
  closeLabel,
  onClose,
  children,
  role = 'dialog',
  width = 440,
}: DialogProps) {
  const id = useId();
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const restore = document.activeElement as HTMLElement | null;
    const first = ref.current?.querySelector<HTMLElement>(
      '[data-autofocus], input, button:not([data-close])',
    );
    first?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener('keydown', onKey, true);
    return () => {
      window.removeEventListener('keydown', onKey, true);
      restore?.focus?.({ preventScroll: true });
    };
  }, [onClose]);

  return createPortal(
    <div
      className={styles.overlay}
      onPointerDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={ref}
        role={role}
        aria-modal="true"
        aria-labelledby={id}
        className={styles.dialog}
        style={{ width: `min(${width}px, 100%)` }}
      >
        <header className={styles.header}>
          <h2 id={id} className={styles.title}>
            {title}
          </h2>
          <button
            type="button"
            data-close
            className={styles.close}
            aria-label={closeLabel}
            onClick={onClose}
          >
            <X size={18} aria-hidden="true" />
          </button>
        </header>
        <div className={styles.body}>{children}</div>
      </div>
    </div>,
    document.body,
  );
}
