import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { commands, unwrap } from '../../api/client';
import { Lock, RefreshCw, Trophy } from 'lucide-react';
import type { AchievementItem } from '../../api/client';
import { useAchievements, useRefreshAchievements } from '../../api/hooks';
import buttons from '../../components/Button/Button.module.css';
import { fr } from '../../i18n/fr';
import { formatLastPlayed } from '../../lib/format';
import { closest } from '../../lib/achievements';
import styles from './GameDetailPage.module.css';

const t = fr.game;
const pct = (p: number) =>
  `${new Intl.NumberFormat('fr-FR', { maximumFractionDigits: 1 }).format(p)} %`;

function Item({ a }: { a: AchievementItem }) {
  const unlocked = !!a.unlockedAt;
  return (
    <li className={`${styles.achievement} ${unlocked ? '' : styles.locked}`}>
      <span className={styles.achievementIcon} aria-hidden="true">
        {unlocked ? <Trophy size={16} /> : <Lock size={14} />}
      </span>
      <span className={styles.achievementText}>
        <span className={styles.achievementName}>{a.name}</span>
        {a.description && <span className={styles.subtle}>{a.description}</span>}
      </span>
      <span className={styles.achievementMeta}>
        {unlocked
          ? t.unlockedOn(formatLastPlayed(a.unlockedAt))
          : a.globalPercent != null
            ? t.rarity(pct(a.globalPercent))
            : ''}
      </span>
    </li>
  );
}

/** `supported`: a source of the game can provide achievements (Steam, GOG, emulator). */
export function AchievementsSection({
  gameId,
  supported,
  isEmulated = false,
}: {
  gameId: string;
  supported: boolean;
  isEmulated?: boolean;
}) {
  const q = useAchievements(gameId, supported);
  const refresh = useRefreshAchievements(gameId);
  const [showAll, setShowAll] = useState(false);
  const v = q.data;

  let body;
  if (!supported || v?.state === 'unsupported') {
    body = <p className={styles.muted}>{t.achievementsUnsupported}</p>;
  } else if (q.isPending) {
    body = <p className={styles.muted}>{t.achievementsLoading}</p>;
  } else if (q.isError || !v) {
    body = <p className={styles.error}>{t.achievementsError}</p>;
  } else if (v.state === 'needsKey') {
    body = <p className={styles.muted}>{t.achievementsNeedsKey}</p>;
  } else if (v.state === 'notMatched') {
    body = <p className={styles.muted}>{t.achievementsNotMatched}</p>;
  } else if (v.state === 'none') {
    body = <p className={styles.muted}>{t.achievementsNone}</p>;
  } else {
    const next = closest(v.items);
    const all = [...v.items].sort(
      (a, b) =>
        Number(!!b.unlockedAt) - Number(!!a.unlockedAt) ||
        (b.unlockedAt ?? '').localeCompare(a.unlockedAt ?? ''),
    );
    body = (
      <>
        <div className={styles.progressRow}>
          <span className="mono">
            {v.unlocked}/{v.total}
          </span>
          <div className={styles.progressBar} aria-hidden="true">
            <span style={{ width: `${v.total ? (v.unlocked / v.total) * 100 : 0}%` }} />
          </div>
        </div>
        {next.length > 0 && (
          <>
            <h3 className={styles.subheading} title={t.achievementsClosestHint}>
              {t.achievementsClosest}
            </h3>
            <ul className={styles.achievements}>
              {next.map((a) => (
                <Item key={a.apiName} a={a} />
              ))}
            </ul>
          </>
        )}
        <button
          type="button"
          className={styles.linkButton}
          aria-expanded={showAll}
          onClick={() => setShowAll(!showAll)}
        >
          {showAll ? t.achievementsHide : t.achievementsAll(v.total)}
        </button>
        {showAll && (
          <ul className={styles.achievements}>
            {all.map((a) => (
              <Item key={a.apiName} a={a} />
            ))}
          </ul>
        )}
      </>
    );
  }

  return (
    <section className={styles.card} aria-labelledby="sheet-achievements">
      <div className={styles.sectionHeader}>
        <h2 id="sheet-achievements" className="section-label">
          {t.achievementsTitle}
        </h2>
        {supported && v && v.state !== 'needsKey' && (
          <button
            type="button"
            className={`${buttons.secondary} ${styles.smallButton}`}
            onClick={() => refresh.mutate()}
            disabled={refresh.isPending}
          >
            <RefreshCw size={14} aria-hidden="true" />
            {t.achievementsRefresh}
          </button>
        )}
      </div>
      {v?.error && <p className={styles.error}>{t.achievementsError}</p>}
      {body}
      {isEmulated && <RaIdForm gameId={gameId} />}
    </section>
  );
}

/** Manual RetroAchievements id (disc consoles, unrecognized ROMs). */
function RaIdForm({ gameId }: { gameId: string }) {
  const client = useQueryClient();
  const [value, setValue] = useState('');
  const save = useMutation({
    mutationFn: () => unwrap(commands.gameSetRaId(gameId, value.trim() ? Number(value) : null)),
    onSuccess: async () => {
      setValue('');
      await unwrap(commands.achievementsGet(gameId, true)).then((data) =>
        client.setQueryData(['achievements', gameId], data),
      );
    },
  });
  return (
    <form
      className={styles.inlineForm}
      onSubmit={(e) => {
        e.preventDefault();
        if (/^\d+$/.test(value.trim())) save.mutate();
      }}
    >
      <label className="visually-hidden" htmlFor="ra-id">
        {t.raId}
      </label>
      <input
        id="ra-id"
        className={styles.input}
        inputMode="numeric"
        placeholder={t.raId}
        value={value}
        onChange={(e) => setValue(e.target.value)}
      />
      <button
        type="submit"
        className={buttons.secondary}
        disabled={!/^\d+$/.test(value.trim()) || save.isPending}
      >
        {t.raIdSave}
      </button>
    </form>
  );
}
