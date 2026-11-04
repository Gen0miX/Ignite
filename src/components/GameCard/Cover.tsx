import { useState } from 'react';
import { artworkSrc } from '../../api/client';
import { titleTint } from '../../lib/format';
import styles from './GameCard.module.css';

interface CoverProps {
  title: string;
  cover?: string | null;
  header?: string | null;
}

/**
 * 2:3 cover. Falls back to the wide Steam header (centred on a blurred copy),
 * then to a tinted placeholder with the title (DESIGN.md › CoverPlaceholder).
 */
export function Cover({ title, cover, header }: CoverProps) {
  const [failed, setFailed] = useState<string | null>(null);
  const coverSrc = artworkSrc(cover);
  const headerSrc = artworkSrc(header);

  if (coverSrc && failed !== coverSrc) {
    return (
      <img
        className={styles.image}
        src={coverSrc}
        alt=""
        loading="lazy"
        decoding="async"
        onError={() => setFailed(coverSrc)}
      />
    );
  }
  if (headerSrc && failed !== headerSrc) {
    return (
      <div className={styles.headerFallback}>
        <img className={styles.headerBlur} src={headerSrc} alt="" aria-hidden="true" />
        <img
          className={styles.headerImage}
          src={headerSrc}
          alt=""
          loading="lazy"
          onError={() => setFailed(headerSrc)}
        />
        <span className={styles.placeholderTitle}>{title}</span>
      </div>
    );
  }
  return (
    <div className={styles.placeholder} style={{ background: titleTint(title) }}>
      <span className={styles.placeholderTitle}>{title}</span>
    </div>
  );
}
