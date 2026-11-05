import { useEffect, useRef, useState, type FormEvent } from 'react';
import { Link } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { X } from 'lucide-react';
import { artworkSrc, commands, unwrap, type ArtworkSlot } from '../../api/client';
import buttons from '../../components/Button/Button.module.css';
import { SegmentedControl } from '../../components/SegmentedControl/SegmentedControl';
import { fr } from '../../i18n/fr';
import styles from './ArtworkPicker.module.css';

const t = fr.artwork;

interface ArtworkPickerProps {
  gameId: string;
  title: string;
  onClose: () => void;
}

/** Pick a cover, hero or logo among SteamGridDB images. */
export function ArtworkPicker({ gameId, title, onClose }: ArtworkPickerProps) {
  const client = useQueryClient();
  const dialogRef = useRef<HTMLDivElement>(null);
  const [slot, setSlot] = useState<ArtworkSlot>('cover');
  const [sgdbGame, setSgdbGame] = useState<number | null>(null);
  const [query, setQuery] = useState(title);
  const [submitted, setSubmitted] = useState<string | null>(null);

  const search = useQuery({
    queryKey: ['artwork-search', gameId, slot, sgdbGame, submitted],
    queryFn: () => unwrap(commands.artworkSearch(gameId, slot, sgdbGame, submitted)),
    staleTime: 10 * 60_000,
  });
  const set = useMutation({
    mutationFn: (url: string) =>
      unwrap(commands.artworkSet(gameId, slot, url, search.data?.selectedGame ?? null)),
    onSuccess: (detail) => {
      client.setQueryData(['game', gameId], detail);
      for (const queryKey of [['library'], ['game']]) void client.invalidateQueries({ queryKey });
      onClose();
    },
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
    body = <p className={styles.muted}>{t.loading}</p>;
  } else if (search.isError || !data) {
    body = <p className={styles.error}>{t.error}</p>;
  } else if (data.state === 'needsKey') {
    body = (
      <p className={styles.muted}>
        {t.needsKey} <Link to="/settings/sources">{t.openSettings}</Link>
      </p>
    );
  } else if (data.state === 'noMatch' || data.options.length === 0) {
    body = <p className={styles.muted}>{t.noResult}</p>;
  } else {
    body = (
      <ul className={`${styles.grid} ${styles[slot]}`}>
        {data.options.map((o) => {
          const src = artworkSrc(o.thumb);
          return (
            <li key={o.url}>
              <button
                type="button"
                className={styles.option}
                disabled={set.isPending}
                onClick={() => set.mutate(o.url)}
                aria-label={t.choose(o.width && o.height ? `${o.width}×${o.height}` : '')}
              >
                {src ? (
                  <img src={src} alt="" loading="lazy" />
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
              label: t.slots[v],
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
          {set.isError && (
            <p role="alert" className={styles.error}>
              {t.setError}
            </p>
          )}
        </div>
        <footer className={styles.footer}>{t.credit}</footer>
      </div>
    </div>
  );
}
