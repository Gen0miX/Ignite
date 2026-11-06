import type { CSSProperties, ReactNode } from 'react';
import styles from './Skeleton.module.css';

interface SkeletonProps {
  width?: CSSProperties['width'];
  height?: CSSProperties['height'];
  /** e.g. "16 / 9" for a media box. */
  aspectRatio?: CSSProperties['aspectRatio'];
  radius?: CSSProperties['borderRadius'];
  className?: string;
}

/** Placeholder block with the size of the content it stands for. */
export function Skeleton({
  width = '100%',
  height,
  aspectRatio,
  radius,
  className,
}: SkeletonProps) {
  return (
    <span
      className={`${styles.block} ${className ?? ''}`}
      style={{
        width,
        height: height ?? (aspectRatio ? 'auto' : undefined),
        aspectRatio,
        borderRadius: radius,
      }}
      aria-hidden="true"
    />
  );
}

/** Loading area: announced once to screen readers, blocks hidden from them. */
export function SkeletonGroup({
  label,
  children,
  className,
}: {
  label: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={`${styles.group} ${className ?? ''}`} role="status" aria-busy="true">
      <span className="visually-hidden">{label}</span>
      {children}
    </div>
  );
}
