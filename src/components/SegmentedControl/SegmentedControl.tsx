import { useRef, type KeyboardEvent } from 'react';
import styles from './SegmentedControl.module.css';

interface Option<T extends string> {
  value: T;
  label: string;
  /** Full name for assistive tech when `label` is abbreviated. */
  ariaLabel?: string;
}

interface SegmentedControlProps<T extends string> {
  label: string;
  options: Option<T>[];
  value: T;
  onChange: (value: T) => void;
  disabled?: boolean;
}

/** Radio group styled as segments; arrows move between options. */
export function SegmentedControl<T extends string>({
  label,
  options,
  value,
  onChange,
  disabled,
}: SegmentedControlProps<T>) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);

  const onKeyDown = (index: number) => (e: KeyboardEvent) => {
    const delta =
      e.key === 'ArrowRight' || e.key === 'ArrowDown'
        ? 1
        : e.key === 'ArrowLeft' || e.key === 'ArrowUp'
          ? -1
          : 0;
    if (!delta) return;
    e.preventDefault();
    const next = (index + delta + options.length) % options.length;
    const option = options[next];
    if (option) {
      onChange(option.value);
      refs.current[next]?.focus();
    }
  };

  return (
    <div role="radiogroup" aria-label={label} className={styles.group}>
      {options.map((o, i) => {
        const checked = o.value === value;
        const fullName = o.ariaLabel && o.ariaLabel !== o.label ? o.ariaLabel : undefined;
        return (
          <button
            key={o.value}
            ref={(el) => {
              refs.current[i] = el;
            }}
            type="button"
            role="radio"
            aria-checked={checked}
            aria-label={fullName}
            title={fullName}
            tabIndex={checked ? 0 : -1}
            disabled={disabled}
            className={`${styles.segment} ${checked ? styles.checked : ''}`}
            onClick={() => onChange(o.value)}
            onKeyDown={onKeyDown(i)}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}
