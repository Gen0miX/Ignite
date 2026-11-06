import { useEffect, useRef, useState, type FormEvent } from 'react';
import { Link } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Check, X } from 'lucide-react';
import { artworkSrc, commands, unwrap, type ArtworkSlot } from '../../api/client';
import buttons from '../../components/Button/Button.module.css';
import { SegmentedControl } from '../../components/SegmentedControl/SegmentedControl';
import { Skeleton, SkeletonGroup } from '../Skeleton/Skeleton';
import { SkeletonImage } from '../Skeleton/SkeletonImage';
import { fr } from '../../i18n/fr';
import styles from './ArtworkPicker.module.css';

const t = fr.artwork;
/** Placeholders shown per type while SteamGridDB answers. */
const SKELETONS: Record<ArtworkSlot, number> = { cover: 12, hero: 6, logo: 8 };
const RATIO: Record<ArtworkSlot, string> = { cover: '2 / 3', hero: '96 / 31', logo: '2 / 1' };

interface ArtworkPickerProps {
  gameId: string;
  title: string;
  onClose: () => void;
}

/** A chosen image, kept until Save. */
interface Pick {
  url: string;
  sgdbGame: number | null;
}

/**
 * Pick a cover, hero and logo among SteamGridDB images: clicks only mark
 * the choice for the current type; Save downloads every chosen image.
 */
export function ArtworkPicker({ gameId, title, onClose }: ArtworkPickerProps) {
  const client = useQueryClient();
  const dialogRef = useRef<HTMLDivElement>(null);
  const [slot, setSlot] = useState<ArtworkSlot>('cover');
  const [sgdbGame, setSgdbGame] = useState<number | null>(null);
  const [query, setQuery] = useState(title);
  const [submitted, setSubmitted] = useState<string | null>(null);

  // One request brings the three image types: switching tabs never reloads,
  // and reopening the picker within 30 minutes reuses the result.
  const search = useQuery({
    queryKey: ['artwork-search', gameId, sgdbGame, submitted],
    queryFn: () => unwrap(commands.artworkSearch(gameId, sgdbGame, submitted)),
    staleTime: 30 * 60_000,
    gcTime: 30 * 60_000,
  });
  const [picks, setPicks] = useState<Partial<Record<ArtworkSlot, Pick>>>({});
  const chosen = Object.entries(picks) as [ArtworkSlot, Pick][];
  const save = useMutation({
    mutationFn: async () => {
      let detail = null;
      for (const [s, p] of chosen) {
        detail = await unwrap(commands.artworkSet(gameId, s, p.url, p.sgdbGame));
      }
      return detail;
    },
    onSuccess: (detail) => {
      if (detail) client.setQueryData(['game', gameId], detail);
      for (const queryKey of [['library'], ['game'], ['home']]) {
        void client.invalidateQueries({ queryKey });
      }
      onClose();
    },
  });
  const toggle = (url: string) =>
    setPicks((p) => {
      const next = { ...p };
      if (next[slot]?.url === url) delete next[slot];
      else next[slot] = { url, sgdbGame: search.data?.selectedGame ?? null };
      return next;
    });

  useEffect(() => {
    dialogRef.current?.querySelector<HTMLElement>('button')?.focus();
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onClose();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    setSgdbGame(null);
    setSubmitted(query);
  };

  const data = search.data;
  let body;
  if (search.isPending) {
    body = (
      <SkeletonGroup label={t.loading}>
        <span className={`${styles.grid} ${styles[slot]}`}>
          {Array.from({ length: SKELETONS[slot] }, (_, i) => (
            <Skeleton key={i} aspectRatio={RATIO[slot]} radius="var(--radius-md)" />
          ))}
        </span>
      </SkeletonGroup>
    );
  } else if (search.isError || !data) {
    body = <p className={styles.error}>{t.error}</p>;
  } else if (data.state === 'needsKey') {
    body = (
      <p className={styles.muted}>
        {t.needsKey} <Link to="/settings/sources">{t.openSettings}</Link>
      </p>
    );
  } else if (data.state === 'noMatch' || data.options[slot].length === 0) {
    body = <p className={styles.muted}>{t.noResult}</p>;
  } else {
    body = (
      <ul className={`${styles.grid} ${styles[slot]}`}>
        {data.options[slot].map((o) => {
          const src = artworkSrc(o.thumb);
          const picked = picks[slot]?.url === o.url;
          return (
            <li key={o.url}>
              <button
                type="button"
                className={`${styles.option} img-host ${picked ? styles.picked : ''}`}
                disabled={save.isPending}
                aria-pressed={picked}
                onClick={() => toggle(o.url)}
                aria-label={t.choose(o.width && o.height ? `${o.width}×${o.height}` : '')}
              >
                {picked && (
                  <span className={styles.pickedMark} aria-hidden="true">
                    <Check size={16} strokeWidth={3} />
                  </span>
                )}
                {src ? (
                  <SkeletonImage src={src} loading="lazy" />
                ) : (
                  <span className={styles.noThumb}>?</span>
                )}
              </button>
            </li>
          );
        })}
      </ul>
    );
  }

  return (
    <div className={styles.overlay} onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="artwork-title"
        className={styles.modal}
      >
        <header className={styles.header}>
          <h2 id="artwork-title">{t.title}</h2>
          <button type="button" className={styles.close} onClick={onClose} aria-label={t.close}>
            <X size={18} aria-hidden="true" />
          </button>
        </header>
        <div className={styles.toolbar}>
          <SegmentedControl
            label={t.slotLabel}
            options={(['cover', 'hero', 'logo'] as const).map((v) => ({
              value: v,
              label: picks[v] ? `${t.slots[v]} ✓` : t.slots[v],
            }))}
            value={slot}
            onChange={setSlot}
          />
          {data && data.games.length > 0 && (
            <label className={styles.gameSelect}>
              <span>{t.game}</span>
              <select
                value={data.selectedGame ?? ''}
                onChange={(e) => setSgdbGame(Number(e.target.value))}
              >
                {data.games.map((g) => (
                  <option key={g.id} value={g.id}>
                    {g.name}
                    {g.year ? ` (${g.year})` : ''}
                  </option>
                ))}
              </select>
            </label>
          )}
          <form className={styles.search} onSubmit={submit}>
            <label className="visually-hidden" htmlFor="artwork-query">
              {t.searchLabel}
            </label>
            <input id="artwork-query" value={query} onChange={(e) => setQuery(e.target.value)} />
            <button type="submit" className={buttons.secondary}>
              {t.search}
            </button>
          </form>
        </div>
        <div className={styles.body}>
          {body}
          {save.isError && (
            <p role="alert" className={styles.error}>
              {t.setError}
            </p>
          )}
        </div>
        <footer className={styles.footer}>
          <span className={styles.credit}>
            {chosen.length > 0
              ? t.chosen(chosen.map(([s]) => t.slots[s].toLowerCase()).join(', '))
              : t.credit}
          </span>
          <button type="button" className={buttons.secondary} onClick={onClose}>
            {t.cancel}
          </button>
          <button
            type="button"
            className={buttons.primary}
            disabled={save.isPending}
            onClick={() => (chosen.length > 0 ? save.mutate() : onClose())}
          >
            {save.isPending ? t.saving : t.save}
          </button>
        </footer>
      </div>
    </div>
  );
}
