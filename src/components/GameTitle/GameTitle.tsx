import { useState } from 'react';
import { artworkSrc } from '../../api/client';
import styles from './GameTitle.module.css';

interface GameTitleProps {
  title: string;
  /** Logo in the artwork cache; the text title is shown without one. */
  logo: string | null | undefined;
  as?: 'h1' | 'h2';
  className?: string;
  size?: 'page' | 'panel';
}

/** Game heading: the game's logo when there is one (alt = title), else its name. */
export function GameTitle({
  title,
  logo,
  as: Tag = 'h2',
  className,
  size = 'panel',
}: GameTitleProps) {
  const src = artworkSrc(logo);
  const [failed, setFailed] = useState<string | null>(null);
  const showLogo = src && failed !== src;
  return (
    <Tag className={`${className ?? ''} ${showLogo ? styles.withLogo : ''}`}>
      {showLogo ? (
        <img
          src={src}
          alt={title}
          className={`${styles.logo} ${styles[size]}`}
          onError={() => setFailed(src)}
        />
      ) : (
        title
      )}
    </Tag>
  );
}
