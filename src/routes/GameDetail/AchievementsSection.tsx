import { useMemo, useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Eye, Lock, RefreshCw, Search, Trophy } from 'lucide-react';
import { artworkSrc, commands, unwrap, type AchievementItem } from '../../api/client';
import { useAchievements, useRefreshAchievements } from '../../api/hooks';
import buttons from '../../components/Button/Button.module.css';
import { SegmentedControl } from '../../components/SegmentedControl/SegmentedControl';
import { Skeleton, SkeletonGroup } from '../../components/Skeleton/Skeleton';
import { fr } from '../../i18n/fr';
import {
  defaultView,
  filterAchievements,
  isMasked,
  monthlyUnlocks,
  rarest,
  rarity,
  rarityCounts,
  sortAchievements,
  type AchSort,
  type AchView,
} from '../../lib/achievements';
import styles from './AchievementsTab.module.css';
import sheet from './GameDetailPage.module.css';

const t = fr.game;
const a = fr.game.ach;
const pct = (p: number) =>
  `${new Intl.NumberFormat('fr-FR', { maximumFractionDigits: 1 }).format(p)} %`;
const dateFmt = new Intl.DateTimeFormat('fr-FR', {
  day: 'numeric',
  month: 'short',
  year: 'numeric',
});
const monthFmt = new Intl.DateTimeFormat('fr-FR', { month: 'long', year: 'numeric' });
/** "Débloqués récemment" at the top of "Tous". */
const RECENT = 4;

/**
 * Achievements tab (docs/FICHE-ONGLETS.md): summary, filters and the list.
 * `sourceLabel`: "Steam", "GOG", "RetroAchievements".
 */
export function AchievementsSection({
  gameId,
  sourceLabel,
  isEmulated = false,
}: {
  gameId: string;
  sourceLabel: string;
  isEmulated?: boolean;
}) {
  const q = useAchievements(gameId);
  const refresh = useRefreshAchievements(gameId);
  const v = q.data;

  let body;
  if (q.isPending) {
    body = (
      <SkeletonGroup label={t.achievementsLoading}>
        <span className={styles.summary}>
          {[0, 1, 2, 3].map((i) => (
            <Skeleton key={i} height={132} radius="var(--radius-xl)" />
          ))}
        </span>
        <span className={styles.list}>
          {[0, 1, 2, 3].map((i) => (
            <Skeleton key={i} height={82} radius="var(--radius-xl)" />
          ))}
        </span>
      </SkeletonGroup>
    );
  } else if (q.isError || !v) {
    body = <p className={sheet.error}>{t.achievementsError}</p>;
  } else if (v.state === 'unsupported') {
    body = <p className={sheet.muted}>{t.achievementsUnsupported}</p>;
  } else if (v.state === 'needsKey') {
    body = <p className={sheet.muted}>{t.achievementsNeedsKey}</p>;
  } else if (v.state === 'notMatched') {
    body = <p className={sheet.muted}>{t.achievementsNotMatched}</p>;
  } else if (v.state === 'none' || v.items.length === 0) {
    body = <p className={sheet.muted}>{t.achievementsNone}</p>;
  } else {
    body = <Achievements key={gameId} items={v.items} sourceLabel={sourceLabel} />;
  }

  return (
    <section className={styles.tab} aria-labelledby="sheet-achievements">
      <div className={sheet.sectionHeader}>
        <h2 id="sheet-achievements" className="visually-hidden">
          {t.achievementsTitle}
        </h2>
        {v?.error && <p className={sheet.error}>{t.achievementsError}</p>}
        {v && v.state !== 'needsKey' && (
          <button
            type="button"
            className={`${buttons.secondary} ${sheet.smallButton} ${styles.refresh}`}
            onClick={() => refresh.mutate()}
            disabled={refresh.isPending}
          >
            <RefreshCw size={14} aria-hidden="true" />
            {t.achievementsRefresh}
          </button>
        )}
      </div>
      {body}
      {isEmulated && <RaIdForm gameId={gameId} />}
    </section>
  );
}

function Achievements({ items, sourceLabel }: { items: AchievementItem[]; sourceLabel: string }) {
  const [view, setView] = useState<AchView>(() => defaultView(items));
  const [sort, setSort] = useState<AchSort>('easiest');
  const [revealAll, setRevealAll] = useState(false);
  const [revealed, setRevealed] = useState<Set<string>>(new Set());
  const [query, setQuery] = useState('');

  const unlocked = items.filter((i) => i.unlockedAt).length;
  const masked = (i: AchievementItem) => isMasked(i, revealAll, revealed);
  const shown = useMemo(
    () => sortAchievements(filterAchievements(items, view, query, masked), sort),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `masked` derives from these.
    [items, view, query, sort, revealAll, revealed],
  );
  // "Tous" without a search: the latest unlocks first, then the rest.
  const recent =
    view === 'all' && !query
      ? sortAchievements(
          items.filter((i) => i.unlockedAt),
          'date',
        ).slice(0, RECENT)
      : [];
  const rest = recent.length ? shown.filter((i) => !recent.includes(i)) : shown;
  const reveal = (id: string) => setRevealed((s) => new Set(s).add(id));

  return (
    <>
      <Summary items={items} unlocked={unlocked} sourceLabel={sourceLabel} />

      <div className={styles.toolbar}>
        <SegmentedControl
          label={a.viewLabel}
          options={[
            { value: 'all', label: a.viewAll(items.length) },
            { value: 'locked', label: a.viewLocked(items.length - unlocked) },
            { value: 'unlocked', label: a.viewUnlocked(unlocked) },
          ]}
          value={view}
          onChange={setView}
        />
        <label className={styles.sort}>
          <span className="visually-hidden">{a.sortLabel}</span>
          <select value={sort} onChange={(e) => setSort(e.target.value as AchSort)}>
            {(['easiest', 'rarest', 'date', 'name'] as const).map((s) => (
              <option key={s} value={s}>
                {a.sort[s]}
              </option>
            ))}
          </select>
        </label>
        <label className={styles.check}>
          <input
            type="checkbox"
            checked={revealAll}
            onChange={(e) => setRevealAll(e.target.checked)}
          />
          {a.revealAll}
        </label>
        <label className={styles.search}>
          <Search size={16} aria-hidden="true" />
          <span className="visually-hidden">{a.search}</span>
          <input
            type="search"
            value={query}
            placeholder={a.search}
            onChange={(e) => setQuery(e.target.value)}
          />
        </label>
      </div>

      {recent.length > 0 && (
        <Group title={a.recent} items={recent} masked={masked} onReveal={reveal} />
      )}
      {rest.length > 0 ? (
        <Group
          title={
            view === 'locked' && sort === 'easiest'
              ? a.easiestTitle
              : recent.length
                ? a.others
                : undefined
          }
          items={rest}
          masked={masked}
          onReveal={reveal}
        />
      ) : (
        recent.length === 0 && <p className={sheet.muted}>{a.noMatch}</p>
      )}
    </>
  );
}

function Summary({
  items,
  unlocked,
  sourceLabel,
}: {
  items: AchievementItem[];
  unlocked: number;
  sourceLabel: string;
}) {
  const ratio = items.length ? unlocked / items.length : 0;
  const counts = rarityCounts(items);
  const months = monthlyUnlocks(items);
  const maxMonth = Math.max(1, ...months.map((m) => m.count));
  const best = rarest(items);
  const totalRarity = Math.max(1, counts.ultra + counts.rare + counts.common);
  return (
    <div className={styles.summary}>
      <div className={styles.card}>
        <div
          className={styles.ring}
          style={{
            background: `conic-gradient(var(--color-accent) ${ratio * 360}deg, var(--color-active) 0)`,
          }}
          role="img"
          aria-label={a.ringLabel(unlocked, items.length)}
        >
          <span className={styles.ringInner}>{Math.floor(ratio * 100)} %</span>
        </div>
        <div>
          <div className={styles.big}>
            {unlocked} / {items.length}
          </div>
          <div className={sheet.muted}>{a.ofSource(sourceLabel)}</div>
        </div>
      </div>

      <div className={styles.card}>
        <h3 className="section-label">{a.byRarity}</h3>
        <div className={styles.stack} aria-hidden="true">
          {(['ultra', 'rare', 'common'] as const).map((r) => (
            <span
              key={r}
              className={styles[`r_${r}`]}
              style={{ width: `${(counts[r] / totalRarity) * 100}%` }}
            />
          ))}
        </div>
        <ul className={styles.legend}>
          {(['ultra', 'rare', 'common'] as const).map((r) => (
            <li key={r}>
              <span className={`${styles.dot} ${styles[`r_${r}`]}`} aria-hidden="true" />
              {a.rarity[r]}
              <span className="mono">{counts[r]}</span>
            </li>
          ))}
        </ul>
      </div>

      <div className={styles.card}>
        <h3 className="section-label">{a.overTime}</h3>
        {months.length > 0 ? (
          <>
            <ol className={styles.months}>
              {months.map((m) => {
                const label = a.monthLabel(
                  monthFmt.format(new Date(`${m.month}-15T12:00:00`)),
                  m.count,
                );
                return (
                  <li key={m.month} title={label} aria-label={label}>
                    <span
                      className={m.recent ? styles.monthRecent : styles.month}
                      style={{
                        height: `${m.count ? Math.max(8, (m.count / maxMonth) * 100) : 0}%`,
                      }}
                    />
                  </li>
                );
              })}
            </ol>
            <div className={styles.monthEnds} aria-hidden="true">
              <span>{months[0]!.month.slice(0, 4)}</span>
              <span>{months[months.length - 1]!.month.slice(0, 4)}</span>
            </div>
          </>
        ) : (
          <p className={sheet.muted}>{a.noUnlock}</p>
        )}
      </div>

      <div className={styles.card}>
        <h3 className="section-label">{a.rarest}</h3>
        {best ? (
          <div className={styles.rarest}>
            <Icon item={best} />
            <div>
              <div className={styles.name}>{best.name}</div>
              <div className={styles[`t_${rarity(best.globalPercent)}`]}>
                {a.players(pct(best.globalPercent!))}
              </div>
            </div>
          </div>
        ) : (
          <p className={sheet.muted}>{a.noUnlock}</p>
        )}
      </div>
    </div>
  );
}

function Group({
  title,
  items,
  masked,
  onReveal,
}: {
  title?: string;
  items: AchievementItem[];
  masked: (i: AchievementItem) => boolean;
  onReveal: (apiName: string) => void;
}) {
  return (
    <section className={styles.group}>
      {title && <h3 className="section-label">{title}</h3>}
      <ul className={styles.list}>
        {items.map((i) => (
          <Item key={i.apiName} item={i} masked={masked(i)} onReveal={() => onReveal(i.apiName)} />
        ))}
      </ul>
    </section>
  );
}

function Icon({ item, masked = false }: { item: AchievementItem; masked?: boolean }) {
  const [failed, setFailed] = useState(false);
  const src = artworkSrc(item.icon);
  const unlocked = !!item.unlockedAt;
  const r = rarity(item.globalPercent);
  return (
    <span
      className={`${styles.icon} ${unlocked ? styles[`b_${r}`] : styles.iconLocked}`}
      aria-hidden="true"
    >
      {masked ? (
        <Eye size={22} />
      ) : src && !failed ? (
        <img src={src} alt="" loading="lazy" onError={() => setFailed(true)} />
      ) : unlocked ? (
        <Trophy size={22} />
      ) : (
        <Lock size={20} />
      )}
    </span>
  );
}

function Item({
  item,
  masked,
  onReveal,
}: {
  item: AchievementItem;
  masked: boolean;
  onReveal: () => void;
}) {
  const unlocked = !!item.unlockedAt;
  const r = rarity(item.globalPercent);
  return (
    <li className={`${styles.item} ${unlocked ? '' : styles.locked}`}>
      <Icon item={item} masked={masked} />
      <div className={styles.text}>
        <div className={styles.nameRow}>
          <span className={styles.name}>{masked ? a.hiddenTitle : item.name}</span>
          {unlocked && item.unlockedAt && (
            <span className={styles.date}>{dateFmt.format(new Date(item.unlockedAt))}</span>
          )}
        </div>
        <p className={styles.desc}>{masked ? a.hiddenBody : item.description}</p>
        {masked ? (
          <button type="button" className={styles.reveal} onClick={onReveal}>
            {a.reveal}
          </button>
        ) : unlocked ? (
          item.globalPercent != null && (
            <span className={styles[`t_${r}`]}>
              {r === 'common'
                ? a.players(pct(item.globalPercent))
                : a.rarityText[r](pct(item.globalPercent))}
            </span>
          )
        ) : (
          item.globalPercent != null && (
            <span className={styles.progress}>
              <span className={styles.track} aria-hidden="true">
                <span style={{ width: `${Math.min(100, item.globalPercent)}%` }} />
              </span>
              <span className={styles.pct}>{a.players(pct(item.globalPercent))}</span>
            </span>
          )
        )}
      </div>
    </li>
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
      className={sheet.inlineForm}
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
        className={sheet.input}
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
