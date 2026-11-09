import { useMemo, useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  CheckCircle2,
  Download,
  Eye,
  FolderOpen,
  Info,
  Lock,
  RefreshCw,
  Search,
  Trophy,
  XCircle,
} from 'lucide-react';
import {
  artworkSrc,
  commands,
  unwrap,
  type AchievementItem,
  type GoldbergStatus,
} from '../../api/client';
import {
  useAchievements,
  useGoldbergClearAppid,
  useGoldbergFindSaves,
  useGoldbergOpenSaves,
  useGoldbergSetup,
  useGoldbergStatus,
  useRefreshAchievements,
} from '../../api/hooks';
import buttons from '../../components/Button/Button.module.css';
import { SegmentedControl } from '../../components/SegmentedControl/SegmentedControl';
import { Skeleton, SkeletonGroup } from '../../components/Skeleton/Skeleton';
import { SkeletonImage } from '../../components/Skeleton/SkeletonImage';
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
 * `sourceLabel`: "Steam", "GOG", "RetroAchievements", "Goldberg".
 */
export function AchievementsSection({
  gameId,
  sourceLabel,
  isEmulated = false,
  isLocal = false,
}: {
  gameId: string;
  sourceLabel: string;
  isEmulated?: boolean;
  isLocal?: boolean;
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
  } else if (v.state === 'needsSetup') {
    body = <p className={sheet.muted}>{t.achievementsNeedsSetup}</p>;
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
      {isLocal && <GoldbergSetup gameId={gameId} initial={v?.goldberg ?? null} />}
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
  const showImage = unlocked && !masked && src && !failed;
  // Downloading into the image cache: the box pulses until the icon arrives.
  const pending = unlocked && !masked && !src && item.iconPending;
  return (
    <span
      className={`${styles.icon} ${unlocked ? styles[`b_${r}`] : styles.iconLocked} img-host ${pending ? 'img-pending' : ''}`}
      aria-hidden="true"
    >
      {masked ? (
        <Eye size={22} />
      ) : !unlocked ? (
        <Lock size={20} />
      ) : showImage ? (
        <SkeletonImage src={src} loading="lazy" onError={() => setFailed(true)} />
      ) : pending ? null : (
        <Trophy size={22} />
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

/** Goldberg setup and installation check for a local game. */
function GoldbergSetup({ gameId, initial }: { gameId: string; initial: GoldbergStatus | null }) {
  const status = useGoldbergStatus(gameId);
  const setup = useGoldbergSetup(gameId);
  const clear = useGoldbergClearAppid(gameId);
  const openSaves = useGoldbergOpenSaves(gameId);
  const findSaves = useGoldbergFindSaves(gameId);
  const [typed, setTyped] = useState<string | null>(null);
  const s = status.data ?? initial;
  const appid = typed ?? (s?.appid ? String(s.appid) : '');

  if (!s) return null;
  const ready = s.installed && s.appid != null && s.definitionsReady;
  const valid = /^\d+$/.test(appid.trim()) && Number(appid) > 0;
  const issues = s.issues.filter((i) => !(ready && (i === 'noSaves' || i === 'needsAppid')));
  const busy = setup.isPending;

  return (
    <section className={`${sheet.card} ${styles.setup}`} aria-labelledby="goldberg-title">
      <div className={sheet.sectionHeader}>
        <h3 id="goldberg-title" className="section-label">
          {t.goldberg.title}
        </h3>
        <span className={ready ? styles.ready : styles.notReady}>
          {ready ? (
            <CheckCircle2 size={16} aria-hidden="true" />
          ) : (
            <XCircle size={16} aria-hidden="true" />
          )}
          {ready
            ? t.goldberg.readyShort
            : s.installed
              ? t.goldberg.installedShort
              : t.goldberg.notInstalled}
        </span>
      </div>
      <p className={sheet.muted}>{t.goldberg.intro}</p>

      <dl className={styles.setupDetails}>
        <dt>{t.goldberg.dll}</dt>
        <dd className="mono">{s.dllPath ?? '—'}</dd>
        <dt>{t.goldberg.settings}</dt>
        <dd className="mono">{s.settingsDir ?? '—'}</dd>
        <dt>{t.goldberg.saves}</dt>
        <dd className="mono">{s.savesPath ?? '—'}</dd>
        <dt>{t.goldberg.arch}</dt>
        <dd>{s.architecture ?? '—'}</dd>
      </dl>

      {issues.length > 0 && (
        <ul className={styles.issues}>
          {issues.map((issue) => (
            <li key={issue}>
              <Info size={14} aria-hidden="true" />
              {t.goldberg.issue[issue]}
            </li>
          ))}
        </ul>
      )}
      {ready && <p className={styles.readyText}>{t.goldberg.ready}</p>}
      {s.appid != null && !s.savesReady && <p className={sheet.muted}>{t.goldberg.savesMissing}</p>}
      {!s.originalBackedUp && s.installed && <p className={sheet.muted}>{t.goldberg.backup}</p>}
      {issues.includes('needsSteamKey') && <p className={sheet.muted}>{t.goldberg.noKey}</p>}

      <form
        className={`${sheet.inlineForm} ${styles.setupForm}`}
        onSubmit={(e) => {
          e.preventDefault();
          if (valid) setup.mutate({ appid: Number(appid), force: s.installed });
        }}
      >
        <label className="visually-hidden" htmlFor="goldberg-appid">
          {t.goldberg.appid}
        </label>
        <input
          id="goldberg-appid"
          className={sheet.input}
          inputMode="numeric"
          placeholder={t.goldberg.appidPlaceholder}
          value={appid}
          onChange={(e) => setTyped(e.target.value)}
          title={t.goldberg.appidHint}
        />
        <button type="submit" className={buttons.secondary} disabled={!valid || setup.isPending}>
          {setup.isPending ? (
            <>
              <RefreshCw size={14} aria-hidden="true" className={styles.spin} />
              {t.goldberg.installing}
            </>
          ) : (
            <>
              <Download size={14} aria-hidden="true" />
              {s.installed ? t.goldberg.reinstall : t.goldberg.install}
            </>
          )}
        </button>
        <button
          type="button"
          className={buttons.secondary}
          onClick={() => void status.refetch()}
          disabled={status.isFetching || busy}
          title={t.goldberg.refresh}
        >
          <RefreshCw
            size={14}
            aria-hidden="true"
            className={status.isFetching ? styles.spin : ''}
          />
          <span className="visually-hidden">{t.goldberg.refresh}</span>
        </button>
        {s.appid != null && (
          <>
            <button
              type="button"
              className={buttons.secondary}
              onClick={() => findSaves.mutate()}
              disabled={busy || findSaves.isPending}
              title={t.goldberg.findSavesHint}
            >
              <Search
                size={14}
                aria-hidden="true"
                className={findSaves.isPending ? styles.spin : ''}
              />
              {t.goldberg.findSaves}
            </button>
            <button
              type="button"
              className={buttons.secondary}
              onClick={() => openSaves.mutate()}
              disabled={busy}
            >
              <FolderOpen size={14} aria-hidden="true" />
              {t.goldberg.openSaves}
            </button>
            <button
              type="button"
              className={buttons.secondary}
              onClick={() => {
                clear.mutate();
                setTyped('');
              }}
              disabled={busy}
            >
              {t.goldberg.clearAppid}
            </button>
          </>
        )}
      </form>
      {findSaves.data && (
        <p
          className={findSaves.data.unlocked.length > 0 ? styles.findFound : sheet.muted}
          role="status"
        >
          {findSaves.data.unlocked.length > 0
            ? t.goldberg.found(findSaves.data.unlocked.length, findSaves.data.paths.length)
            : t.goldberg.foundNone}
        </p>
      )}
      {findSaves.data && findSaves.data.paths.length > 0 && (
        <ul className={styles.findPaths}>
          {findSaves.data.paths.map((path) => (
            <li key={path} className="mono">
              {path}
            </li>
          ))}
        </ul>
      )}
      {(setup.isError || clear.isError || findSaves.isError) && (
        <p role="alert" className={sheet.error}>
          {t.goldberg.error}
        </p>
      )}
    </section>
  );
}
