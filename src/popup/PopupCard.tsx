import { useState } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { Flame, Timer, Trophy } from 'lucide-react';
import type { Popup } from '../api/bindings';
import { isTauri } from '../api/client';
import { IgniteMark } from '../components/AppLogo/AppLogo';
import { fr } from '../i18n/fr';
import { rarity } from '../lib/achievements';
import styles from './PopupCard.module.css';

const t = fr.popup;

const percentFmt = new Intl.NumberFormat('fr-FR', { maximumFractionDigits: 1 });

/** Rarity badge text: « Ultra-rare · 2,1 % », « Rare · 12 % », « 48 % des joueurs ». */
export function rarityLabel(percent: number): string {
  const p = `${percentFmt.format(percent)} %`;
  const r = rarity(percent);
  return r === 'common' ? t.players(p) : t.rarity[r](p);
}

/** `asset:` URL for a local file, the URL itself for a remote image. */
export function imageSrc(image: string | null | undefined): string | undefined {
  if (!image) return undefined;
  if (/^https?:\/\//.test(image)) return image;
  return isTauri() ? convertFileSrc(image) : undefined;
}

function label(popup: Popup): string {
  switch (popup.kind) {
    case 'achievements':
      return t.achievements(popup.count ?? 0);
    case 'session':
      return t.session;
    default:
      return t.achievement;
  }
}

interface PopupCardProps {
  popup: Popup;
  /** Display time in ms: drives the progress line. */
  duration: number;
  leaving: boolean;
}

/** Ignite notification card: image, kind label, title, details, game. */
export function PopupCard({ popup, duration, leaving }: PopupCardProps) {
  const [broken, setBroken] = useState(false);
  const src = broken ? undefined : imageSrc(popup.image);
  const session = popup.kind === 'session';
  const title = popup.title;
  const description = popup.description;
  // The game line is redundant when the title already is the game.
  const game = popup.game && popup.game !== title ? popup.game : null;
  const FallbackIcon = session ? Timer : Trophy;

  return (
    <div
      className={`${styles.card} ${leaving ? styles.leaving : ''}`}
      role="status"
      aria-live="polite"
    >
      <div className={`${styles.media} ${session ? styles.mediaSession : ''}`}>
        {src ? (
          <img src={src} alt="" className={styles.image} onError={() => setBroken(true)} />
        ) : (
          <FallbackIcon size={30} aria-hidden="true" className={styles.fallback} />
        )}
      </div>
      <div className={styles.body}>
        <p className={styles.kind}>
          <Flame size={12} aria-hidden="true" />
          {label(popup)}
        </p>
        <p className={styles.title}>{title}</p>
        {description && <p className={styles.description}>{description}</p>}
        {(game || popup.percent != null) && (
          <p className={styles.meta}>
            {game && <span className={styles.game}>{game}</span>}
            {popup.percent != null && (
              <span className={`${styles.rarity} ${styles[rarity(popup.percent)]}`}>
                {rarityLabel(popup.percent)}
              </span>
            )}
          </p>
        )}
      </div>
      <span className={styles.mark} aria-hidden="true">
        <IgniteMark size={20} />
      </span>
      <span
        className={styles.progress}
        style={{ animationDuration: `${duration}ms` }}
        aria-hidden="true"
      />
    </div>
  );
}
