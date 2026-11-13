import { useLayoutEffect, useMemo, useRef, useState } from 'react';
import { Link } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import {
  artworkSrc,
  commands,
  isTauri,
  unwrap,
  type SourceKind,
  type StatsRange,
} from '../../api/client';
import { PageMain } from '../../app/AppShell';
import { EmptyState } from '../../components/EmptyState/EmptyState';
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { SegmentedControl } from '../../components/SegmentedControl/SegmentedControl';
import { SourceBadge } from '../../components/SourceBadge/SourceBadge';
import { locale, strings } from '../../i18n';
import { buildCalendar, calendarCell, monthStarts } from '../../lib/calendar';
import { formatPlaytimeLong, titleTint } from '../../lib/format';
import { defaultStatsRange, usePrefs } from '../../lib/prefs';
import styles from './StatsPage.module.css';

const t = strings.stats;
const ALL_SOURCES: SourceKind[] = [
  'steam',
  'gog',
  'epic',
  'battlenet',
  'ea',
  'ubisoft',
  'xbox',
  'riot',
  'local',
  'emulator',
];
const monthShort = new Intl.DateTimeFormat(locale, { month: 'short' });
/** Month index (0-11) → short name, e.g. « janv. ». */
const MONTHS = Array.from({ length: 12 }, (_, m) => monthShort.format(new Date(2024, m, 15)));

const hours = (minutes: number) =>
  minutes >= 60 ? strings.format.hours(Math.round(minutes / 60)) : strings.format.minutes(minutes);
const longDate = new Intl.DateTimeFormat(locale, {
  day: 'numeric',
  month: 'long',
  year: 'numeric',
});
const monthName = new Intl.DateTimeFormat(locale, { month: 'long' });

function iso(d: Date) {
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

function SourcesFilter({
  value,
  onChange,
}: {
  value: SourceKind[];
  onChange: (v: SourceKind[]) => void;
}) {
  const label = value.length === 0 ? t.allSources : value.map((k) => strings.sources[k]).join(', ');
  return (
    <details className={styles.sources}>
      <summary>{t.sourcesLabel(label)}</summary>
      <div className={styles.sourcesMenu} role="group" aria-label={t.sourcesFilter}>
        {ALL_SOURCES.map((k) => (
          <label key={k}>
            <input
              type="checkbox"
              checked={value.includes(k)}
              onChange={(e) =>
                onChange(e.target.checked ? [...value, k] : value.filter((x) => x !== k))
              }
            />
            {strings.sources[k]}
          </label>
        ))}
        {value.length > 0 && (
          <button type="button" className={styles.linkButton} onClick={() => onChange([])}>
            {t.allSources}
          </button>
        )}
      </div>
    </details>
  );
}

function Kpi({ label, value, detail }: { label: string; value: string; detail?: string }) {
  return (
    <div className={styles.kpi}>
      <span className={styles.kpiLabel}>{label}</span>
      <span className={styles.kpiValue}>{value}</span>
      {detail && <span className={styles.kpiDetail}>{detail}</span>}
    </div>
  );
}

const CELL_GAP = 3;

function Calendar({
  days,
  start,
  end,
}: {
  days: { day: string; minutes: number }[];
  start: string;
  end: string;
}) {
  const weeks = useMemo(() => buildCalendar(days, start, end), [days, start, end]);
  const months = monthStarts(weeks);
  const wrap = useRef<HTMLDivElement>(null);
  const [cell, setCell] = useState(14);

  // Cells sized so the year fills the card (10–18 px), recomputed on resize.
  useLayoutEffect(() => {
    const el = wrap.current;
    if (!el) return;
    const measure = () => setCell(calendarCell(el.clientWidth, weeks.length, CELL_GAP));
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [weeks.length]);

  return (
    <div
      ref={wrap}
      className={styles.calendarWrap}
      style={{ '--cell': `${cell}px`, '--cell-gap': `${CELL_GAP}px` } as React.CSSProperties}
    >
      <div
        className={styles.monthRow}
        aria-hidden="true"
        style={{ gridTemplateColumns: `repeat(${weeks.length}, var(--cell))` }}
      >
        {months.map((m) => (
          <span key={`${m.week}-${m.month}`} style={{ gridColumn: m.week + 1 }}>
            {MONTHS[m.month]}
          </span>
        ))}
      </div>
      <div className={styles.calendar} role="img" aria-label={t.calendarLabel}>
        {weeks.map((week, i) => (
          <div key={i} className={styles.week}>
            {week.map((c, j) => (
              <span
                key={j}
                className={`${styles.cell} ${c.day ? styles[`heat${c.level}`] : styles.pad}`}
                title={
                  c.day
                    ? `${longDate.format(new Date(`${c.day}T12:00:00`))} : ${c.minutes ? formatPlaytimeLong(c.minutes) : t.noPlay}`
                    : undefined
                }
              />
            ))}
          </div>
        ))}
      </div>
      <div className={styles.legend} aria-hidden="true">
        <span>{t.less}</span>
        {[0, 1, 2, 3].map((l) => (
          <span key={l} className={`${styles.cell} ${styles[`heat${l}`]}`} />
        ))}
        <span>{t.more}</span>
      </div>
    </div>
  );
}

export function StatsPage() {
  const picked = usePrefs((p) => p.statsRange);
  const setPicked = usePrefs((p) => p.setStatsRange);
  // Until the user picks a period: "Depuis toujours" first (it carries the
  // first tracked day), then the year if Ignite's history is old enough.
  const [auto, setAuto] = useState<StatsRange>('all');
  const range = picked ?? auto;
  const setRange = setPicked;
  const [sources, setSources] = useState<SourceKind[]>([]);
  const stats = useQuery({
    queryKey: ['stats', range, sources],
    queryFn: () => unwrap(commands.statsOverview(range, sources)),
    enabled: isTauri(),
    placeholderData: (prev) => prev,
  });
  const firstTracked = stats.data?.firstTrackedDay;
  const suggested = stats.data ? defaultStatsRange(firstTracked ?? null) : null;
  if (!picked && suggested && suggested !== auto) setAuto(suggested);

  const now = new Date();
  const year = now.getFullYear();
  const rangeLabels: Record<StatsRange, string> = {
    week: t.week,
    month: t.month,
    year: String(year),
    all: t.all,
  };

  if (!isTauri()) {
    return (
      <PageMain>
        <PageHeader title={t.title} />
        <EmptyState>{strings.library.backendUnavailable}</EmptyState>
      </PageMain>
    );
  }

  const s = stats.data;
  const calStart =
    range === 'year'
      ? `${year}-01-01`
      : iso(new Date(now.getFullYear(), now.getMonth(), now.getDate() - 364));
  const calEnd = range === 'year' ? `${year}-12-31` : iso(now);
  const topMax = Math.max(1, ...(s?.topGames.map((g) => g.minutes) ?? [1]));
  const sourceTotal = Math.max(1, s?.bySource.reduce((a, b) => a + b.minutes, 0) ?? 1);
  const lib = s?.library;
  const libTotal = lib ? Math.max(1, lib.done + lib.playing + lib.dropped + lib.todo) : 1;

  return (
    <PageMain>
      <PageHeader title={t.title} />
      <div className={styles.toolbar}>
        <SegmentedControl
          label={t.period}
          options={(['week', 'month', 'year', 'all'] as const).map((v) => ({
            value: v,
            label: rangeLabels[v],
          }))}
          value={range}
          onChange={setRange}
        />
        <SourcesFilter value={sources} onChange={setSources} />
      </div>

      {!s ? (
        <p className={styles.muted}>{t.loading}</p>
      ) : (
        <>
          <div className={styles.kpis}>
            <Kpi
              label={t.timePlayed(rangeLabels[range])}
              value={hours(s.totalMinutes)}
              detail={
                s.minutesPerDay > 0 ? t.perDay(formatPlaytimeLong(s.minutesPerDay)) : undefined
              }
            />
            <Kpi
              label={t.gamesPlayed}
              value={String(s.gamesPlayed)}
              detail={t.owned(s.gamesOwned)}
            />
            <Kpi label={t.gamesCompleted} value={String(s.gamesCompleted)} />
            <Kpi
              label={t.achievements}
              value={String(s.achievementsUnlocked)}
              detail={t.achievementsDetail}
            />
            <Kpi
              label={t.streak}
              value={t.days(s.longestStreakDays)}
              detail={
                s.longestStreakEnd
                  ? t.streakIn(monthName.format(new Date(`${s.longestStreakEnd}T12:00:00`)))
                  : undefined
              }
            />
          </div>

          {s.historyStart && (
            <p className={styles.muted}>
              {t.historyNote(longDate.format(new Date(`${s.historyStart}T12:00:00`)))}
            </p>
          )}

          <section className={styles.card} aria-labelledby="stats-calendar">
            <h2 id="stats-calendar" className="section-label">
              {t.calendar}
            </h2>
            {range !== 'all' && s.sessionsInRange < 3 ? (
              <p className={styles.youngHistory}>
                {t.youngHistory(
                  longDate.format(new Date(`${s.firstTrackedDay ?? iso(now)}T12:00:00`)),
                )}
              </p>
            ) : (
              <Calendar days={s.calendar} start={calStart} end={calEnd} />
            )}
          </section>

          <div className={styles.columns}>
            <section className={styles.card} aria-labelledby="stats-top">
              <h2 id="stats-top" className="section-label">
                {t.topGames(rangeLabels[range])}
              </h2>
              {s.topGames.length === 0 ? (
                <p className={styles.muted}>{t.empty}</p>
              ) : (
                <ol className={styles.top}>
                  {s.topGames.map((g, i) => {
                    const img = artworkSrc(g.cover ?? g.header);
                    return (
                      <li key={g.gameId}>
                        <span className={styles.rank}>{i + 1}</span>
                        <span
                          className={styles.thumb}
                          style={img ? undefined : { background: titleTint(g.title) }}
                        >
                          {img && <img src={img} alt="" />}
                        </span>
                        <span className={styles.topBody}>
                          <span className={styles.topTitle}>
                            <Link to={`/game/${g.gameId}`}>{g.title}</Link>
                            {g.source && <SourceBadge kind={g.source} />}
                          </span>
                          <span className={styles.bar} aria-hidden="true">
                            <span
                              style={{
                                width: `${(g.minutes / topMax) * 100}%`,
                                background: g.source
                                  ? `var(--source-${g.source})`
                                  : 'var(--color-muted)',
                              }}
                            />
                          </span>
                        </span>
                        <span className="mono">{hours(g.minutes)}</span>
                      </li>
                    );
                  })}
                </ol>
              )}
            </section>

            <div className={styles.side}>
              <section className={styles.card} aria-labelledby="stats-sources">
                <h2 id="stats-sources" className="section-label">
                  {t.bySource}
                </h2>
                {s.bySource.length === 0 ? (
                  <p className={styles.muted}>{t.empty}</p>
                ) : (
                  <>
                    <div className={styles.stack} aria-hidden="true">
                      {s.bySource.map((p) => (
                        <span
                          key={p.kind ?? 'other'}
                          style={{
                            width: `${(p.minutes / sourceTotal) * 100}%`,
                            background: p.kind ? `var(--source-${p.kind})` : 'var(--color-muted)',
                          }}
                        />
                      ))}
                    </div>
                    <ul className={styles.legendList}>
                      {s.bySource.map((p) => (
                        <li key={p.kind ?? 'other'}>
                          <span
                            className={styles.dot}
                            style={{
                              background: p.kind ? `var(--source-${p.kind})` : 'var(--color-muted)',
                            }}
                          />
                          <span>{p.kind ? strings.sources[p.kind] : t.otherSessions}</span>
                          <span className="mono">{hours(p.minutes)}</span>
                        </li>
                      ))}
                    </ul>
                  </>
                )}
              </section>

              {lib && (
                <section className={styles.card} aria-labelledby="stats-library">
                  <h2 id="stats-library" className="section-label">
                    {t.library}
                  </h2>
                  <div className={styles.stack} aria-hidden="true">
                    {(['done', 'playing', 'dropped', 'todo'] as const).map((k) => (
                      <span
                        key={k}
                        style={{
                          width: `${(lib[k] / libTotal) * 100}%`,
                          background: `var(--status-${k})`,
                        }}
                      />
                    ))}
                  </div>
                  <ul className={styles.legendList}>
                    {(['done', 'playing', 'dropped', 'todo'] as const).map((k) => (
                      <li key={k}>
                        <span className={styles.dot} style={{ background: `var(--status-${k})` }} />
                        <span>{t.libraryLabels[k]}</span>
                        <span className="mono">{lib[k]}</span>
                      </li>
                    ))}
                  </ul>
                </section>
              )}
            </div>
          </div>
        </>
      )}
    </PageMain>
  );
}
