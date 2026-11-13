import { useState, type FormEvent } from 'react';
import { Link, Navigate, NavLink, useParams } from 'react-router-dom';
import {
  Camera,
  EyeOff,
  Gamepad2,
  Info,
  KeyRound,
  Library,
  SlidersHorizontal,
  Trophy,
  type LucideIcon,
} from 'lucide-react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  commands,
  isTauri,
  unwrap,
  type AppSettings,
  type SecretKind,
  type SourceCard,
  type SourceStatus,
} from '../../api/client';
import {
  useBulkUpdate,
  useClearSecret,
  useLibraryList,
  useSetSecret,
  useSecretsStatus,
  useSetSourceEnabled,
  useSetTrackingSettings,
  useTrackingSettings,
  useSetSteamId,
  useSourcesStatus,
  useSteamSettings,
  useSync,
} from '../../api/hooks';
import { PageMain } from '../../app/AppShell';
import buttons from '../../components/Button/Button.module.css';
import { EmptyState } from '../../components/EmptyState/EmptyState';
import { Cover } from '../../components/GameCard/Cover';
import { GoldbergOptions } from '../../components/GoldbergOptions/GoldbergOptions';
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { SourceChip } from '../../components/SourceBadge/SourceBadge';
import { SegmentedControl } from '../../components/SegmentedControl/SegmentedControl';
import { Switch } from '../../components/Switch/Switch';
import { fr } from '../../i18n/fr';
import { formatLastPlayed } from '../../lib/format';
import { usePrefs, type RecapMode } from '../../lib/prefs';
import { useUpdates } from '../../lib/updates';
import styles from './SettingsSourcesPage.module.css';
import { CaptureFoldersCard } from './CaptureFoldersCard';
import { EmulatorsCard, RaAccount } from './EmulatorsCard';

const t = fr.settings;

function StatusPill({ status }: { status: SourceStatus }) {
  const tone =
    status.state === 'connected'
      ? styles.ok
      : status.state === 'error'
        ? styles.danger
        : styles.warn;
  return <span className={`${styles.pill} ${tone}`}>{t.status[status.state]}</span>;
}

function SteamCard({ card }: { card: SourceCard }) {
  const settings = useSteamSettings();
  const sync = useSync();
  const syncing = sync.isPending || card.syncing;
  const report = card.lastSync;

  return (
    <section className={styles.card} aria-labelledby="source-steam">
      <header className={styles.cardHeader}>
        <SourceChip kind="steam" />
        <div className={styles.cardTitle}>
          <h3 id="source-steam">{t.steam.title}</h3>
          <span className={styles.muted}>{t.steam.provides}</span>
        </div>
        <StatusPill status={card.status} />
        <SourceSwitch card={card} />
      </header>

      {card.status.state === 'error' && (
        <p role="alert" className={styles.errorText}>
          {card.status.detail} — {t.steam.privateProfileHint}
        </p>
      )}

      <dl className={styles.details}>
        <dt>{t.steam.path}</dt>
        <dd className="mono">{card.location ?? '—'}</dd>
        <dt>{t.lastSync}</dt>
        <dd>
          {report
            ? `${t.syncSummary(report.total, report.added)} · ${formatLastPlayed(report.finishedAt)}`
            : t.neverSynced}
        </dd>
      </dl>
      {!card.location && <p className={styles.muted}>{t.steam.notFound}</p>}
      {report?.warnings.map((w) => (
        <p key={w} className={styles.warnText}>
          {w}
        </p>
      ))}

      {settings.data && (
        <p className={styles.muted}>
          {settings.data.apiKeyPresent ? t.steam.accountsLinkReady : t.steam.accountsLinkMissing}{' '}
          <Link to="/settings/accounts" className={styles.inlineLink}>
            {t.nav.accounts}
          </Link>
        </p>
      )}

      <div>
        <button
          type="button"
          className={buttons.secondary}
          onClick={() => sync.mutate()}
          disabled={syncing}
        >
          {syncing ? t.syncing : t.resync}
        </button>
      </div>
    </section>
  );
}

function SourceSwitch({ card }: { card: SourceCard }) {
  const toggle = useSetSourceEnabled();
  const name = fr.sources[card.kind];
  return (
    <Switch
      label={card.enabled ? t.disable(name) : t.enable(name)}
      checked={card.enabled}
      disabled={toggle.isPending || card.syncing}
      onChange={(enabled) => toggle.mutate({ kind: card.kind, enabled })}
    />
  );
}

const DETAILS: Partial<Record<SourceCard['kind'], { provides: string; note: string }>> = {
  gog: t.gog,
  epic: t.epic,
  riot: { provides: t.secondary.provides, note: t.secondary.riotNote },
  battlenet: t.secondary,
  ea: t.secondary,
  ubisoft: t.secondary,
};

/** Card for launchers without extra settings (GOG, Epic). */
function GenericSourceCard({ card }: { card: SourceCard }) {
  const sync = useSync();
  const name = fr.sources[card.kind];
  const info = DETAILS[card.kind];
  const report = card.lastSync;
  const installed = card.status.state !== 'not_installed';
  return (
    <section className={styles.card} aria-labelledby={`source-${card.kind}`}>
      <header className={styles.cardHeader}>
        <SourceChip kind={card.kind} />
        <div className={styles.cardTitle}>
          <h3 id={`source-${card.kind}`}>{name}</h3>
          {info && <span className={styles.muted}>{info.provides}</span>}
        </div>
        <StatusPill status={card.status} />
        {installed && <SourceSwitch card={card} />}
      </header>
      {!installed ? (
        <p className={styles.muted}>{t.notInstalledHint(name)}</p>
      ) : (
        <>
          {card.status.state === 'error' && (
            <p role="alert" className={styles.errorText}>
              {card.status.detail}
            </p>
          )}
          <dl className={styles.details}>
            <dt>{t.location}</dt>
            <dd className="mono">{card.location ?? '—'}</dd>
            <dt>{t.lastSync}</dt>
            <dd>
              {report
                ? `${t.syncSummary(report.total, report.added)} · ${formatLastPlayed(report.finishedAt)}`
                : t.neverSynced}
            </dd>
          </dl>
          {!card.enabled && <p className={styles.muted}>{t.disabledHint}</p>}
          {info && <p className={styles.muted}>{info.note}</p>}
          {card.enabled && (
            <div>
              <button
                type="button"
                className={buttons.secondary}
                onClick={() => sync.mutate()}
                disabled={sync.isPending || card.syncing}
              >
                {sync.isPending || card.syncing ? t.syncing : t.resync}
              </button>
            </div>
          )}
        </>
      )}
    </section>
  );
}

const MANUAL = '__manual__';
const AUTO = '';

function SteamAccount() {
  const { data } = useSteamSettings();
  const setId = useSetSteamId();
  const [choice, setChoice] = useState<string>(
    data?.steamIdConfigured
      ? data.accounts.some((a) => a.steamId === data.steamId)
        ? (data.steamId ?? AUTO)
        : MANUAL
      : AUTO,
  );
  const [manual, setManual] = useState(
    data?.steamIdConfigured && !data.accounts.some((a) => a.steamId === data.steamId)
      ? (data.steamId ?? '')
      : '',
  );
  if (!data) return null;
  const detected = data.accounts[0]?.steamId;

  const onChange = (value: string) => {
    setChoice(value);
    if (value !== MANUAL) setId.mutate(value === AUTO ? null : value);
  };
  const onManual = (e: FormEvent) => {
    e.preventDefault();
    setId.mutate(manual);
  };

  return (
    <div className={styles.field}>
      <label htmlFor="steam-account" className={styles.label}>
        {t.steam.account}
      </label>
      <select
        id="steam-account"
        className={styles.select}
        value={choice}
        onChange={(e) => onChange(e.target.value)}
      >
        <option value={AUTO}>
          {detected ? t.steam.accountAuto(detected) : t.steam.accountAutoNone}
        </option>
        {data.accounts.map((a) => (
          <option key={a.steamId} value={a.steamId}>
            {a.personaName ? `${a.personaName} (${a.steamId})` : a.steamId}
          </option>
        ))}
        <option value={MANUAL}>{t.steam.accountManual}</option>
      </select>
      {choice === MANUAL && (
        <form className={styles.inline} onSubmit={onManual}>
          <label className="visually-hidden" htmlFor="steam-id-input">
            {t.steam.accountInputLabel}
          </label>
          <input
            id="steam-id-input"
            className={`${styles.input} mono`}
            inputMode="numeric"
            placeholder="7656119…"
            value={manual}
            onChange={(e) => setManual(e.target.value)}
          />
          <button type="submit" className={buttons.secondary} disabled={setId.isPending}>
            {t.steam.accountSave}
          </button>
        </form>
      )}
      {setId.isError && (
        <p role="alert" className={styles.errorText}>
          {t.steam.accountInvalid}
        </p>
      )}
    </div>
  );
}

function SteamApiKey({ present }: { present: boolean }) {
  const setSecret = useSetSecret();
  const clearSecret = useClearSecret();
  const [value, setValue] = useState('');

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (!value.trim()) return;
    setSecret.mutate({ kind: 'steam_web_api_key', value }, { onSuccess: () => setValue('') });
  };

  return (
    <div className={styles.field}>
      <span className={styles.label} id="steam-key-label">
        {t.steam.apiKey}
        <span className={`${styles.pill} ${present ? styles.ok : styles.neutral}`}>
          {present ? t.steam.apiKeyPresent : t.steam.apiKeyMissing}
        </span>
      </span>
      <p className={styles.muted}>{t.steam.apiKeyHelp}</p>
      <form className={styles.inline} onSubmit={onSubmit} aria-labelledby="steam-key-label">
        <label className="visually-hidden" htmlFor="steam-key-input">
          {t.steam.apiKeyInput}
        </label>
        <input
          id="steam-key-input"
          className={`${styles.input} mono`}
          type="password"
          autoComplete="off"
          spellCheck={false}
          placeholder={t.steam.apiKeyInput}
          value={value}
          onChange={(e) => setValue(e.target.value)}
        />
        <button
          type="submit"
          className={buttons.primary}
          disabled={!value.trim() || setSecret.isPending}
        >
          {t.steam.apiKeySave}
        </button>
        {present && (
          <button
            type="button"
            className={buttons.secondary}
            onClick={() => clearSecret.mutate('steam_web_api_key')}
            disabled={clearSecret.isPending}
          >
            {t.steam.apiKeyClear}
          </button>
        )}
      </form>
      {setSecret.isError && (
        <p role="alert" className={styles.errorText}>
          {t.steam.apiKeyError}
        </p>
      )}
    </div>
  );
}

/** Write-only key field: the value is never read back from the vault. */
function SecretKeyField({ kind, labelId }: { kind: SecretKind; labelId: string }) {
  const status = useSecretsStatus();
  const setSecret = useSetSecret();
  const clearSecret = useClearSecret();
  const [value, setValue] = useState('');
  const present = status.data?.find((s) => s.kind === kind)?.present ?? false;

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (!value.trim()) return;
    setSecret.mutate({ kind, value }, { onSuccess: () => setValue('') });
  };

  return (
    <>
      <span className={`${styles.pill} ${present ? styles.ok : styles.neutral}`}>
        {present ? t.key.present : t.key.missing}
      </span>
      <form className={styles.inline} onSubmit={onSubmit} aria-labelledby={labelId}>
        <label className="visually-hidden" htmlFor={`${kind}-input`}>
          {t.key.input}
        </label>
        <input
          id={`${kind}-input`}
          className={`${styles.input} mono`}
          type="password"
          autoComplete="off"
          spellCheck={false}
          placeholder={t.key.input}
          value={value}
          onChange={(e) => setValue(e.target.value)}
        />
        <button
          type="submit"
          className={buttons.primary}
          disabled={!value.trim() || setSecret.isPending}
        >
          {t.key.save}
        </button>
        {present && (
          <button
            type="button"
            className={buttons.secondary}
            onClick={() => clearSecret.mutate(kind)}
            disabled={clearSecret.isPending}
          >
            {t.key.clear}
          </button>
        )}
      </form>
      {setSecret.isError && (
        <p role="alert" className={styles.errorText}>
          {t.key.error}
        </p>
      )}
    </>
  );
}

const MIN_SESSION_CHOICES = [0, 1, 2, 5, 10];
const IDLE_CHOICES = [5, 10, 15, 30, 60];

function TrackingCard() {
  const { data } = useTrackingSettings();
  const save = useSetTrackingSettings();
  if (!data) return null;
  const tt = t.tracking;
  const minutes = (n: number) => (n === 0 ? tt.none : tt.minutes(n));
  const withCurrent = (choices: number[], v: number) =>
    choices.includes(v) ? choices : [...choices, v].sort((a, b) => a - b);
  return (
    <section className={styles.card} aria-labelledby="tracking-title">
      <h3 id="tracking-title" className="section-label">
        {t.trackingTitle}
      </h3>
      <div className={styles.field}>
        <label className={styles.label} htmlFor="min-session">
          {tt.minSession}
        </label>
        <select
          id="min-session"
          className={styles.select}
          value={data.minSessionMinutes}
          onChange={(e) => save.mutate({ ...data, minSessionMinutes: Number(e.target.value) })}
        >
          {withCurrent(MIN_SESSION_CHOICES, data.minSessionMinutes).map((n) => (
            <option key={n} value={n}>
              {minutes(n)}
            </option>
          ))}
        </select>
        <span className={styles.muted}>{tt.minSessionHint}</span>
      </div>
      <div className={styles.field}>
        <label className={styles.label} htmlFor="idle">
          {tt.idle}
        </label>
        <select
          id="idle"
          className={styles.select}
          value={data.idleMinutes}
          onChange={(e) => save.mutate({ ...data, idleMinutes: Number(e.target.value) })}
        >
          {withCurrent(IDLE_CHOICES, data.idleMinutes).map((n) => (
            <option key={n} value={n}>
              {minutes(n)}
            </option>
          ))}
        </select>
        <span className={styles.muted}>{tt.idleHint}</span>
      </div>
      <details className={styles.howItWorks}>
        <summary>{tt.howItWorks}</summary>
        <dl className={styles.details}>
          <dt>{tt.method}</dt>
          <dd>{tt.methodValue}</dd>
          <dt>{tt.duplicates}</dt>
          <dd>{tt.duplicatesValue}</dd>
          <dt>{tt.history}</dt>
          <dd>{tt.historyValue}</dd>
        </dl>
      </details>
      {save.isError && (
        <p role="alert" className={styles.errorText}>
          {tt.saveError}
        </p>
      )}
    </section>
  );
}

/** Ignite's own success notifications (popup in the corner of the screen). */
function NotificationsCard() {
  const { data } = useTrackingSettings();
  const save = useSetTrackingSettings();
  if (!data) return null;
  const tt = t.tracking;
  return (
    <section className={styles.card} aria-labelledby="notifications-title">
      <h3 id="notifications-title" className="section-label">
        {tt.notificationsTitle}
      </h3>
      <div className={styles.cardHeader}>
        <div className={styles.cardTitle}>
          <span className={styles.label}>{tt.toasts}</span>
          <span className={styles.muted}>{tt.toastsHint}</span>
        </div>
        <Switch
          label={tt.toasts}
          checked={data.achievementToasts}
          onChange={(achievementToasts) => save.mutate({ ...data, achievementToasts })}
        />
      </div>
      {save.isError && (
        <p role="alert" className={styles.errorText}>
          {tt.saveError}
        </p>
      )}
    </section>
  );
}

/** Display preferences (stored in the WebView, no backend). */
function GeneralCard() {
  const showTrailers = usePrefs((p) => p.showTrailers);
  const setShowTrailers = usePrefs((p) => p.setShowTrailers);
  const recapMode = usePrefs((p) => p.recapMode);
  const recapMinMinutes = usePrefs((p) => p.recapMinMinutes);
  const setRecap = usePrefs((p) => p.setRecap);
  const tg = t.general;
  return (
    <section className={styles.card} aria-labelledby="general-title">
      <h3 id="general-title" className="section-label">
        {tg.title}
      </h3>
      <div className={styles.cardHeader}>
        <div className={styles.cardTitle}>
          <span className={styles.label}>{tg.showTrailers}</span>
          <span className={styles.muted}>{tg.showTrailersHint}</span>
        </div>
        <Switch label={tg.showTrailers} checked={showTrailers} onChange={setShowTrailers} />
      </div>
      <h3 className={styles.label}>{tg.endTitle}</h3>
      <span className={styles.muted}>{tg.recapHint}</span>
      <SegmentedControl
        label={tg.recapLabel}
        options={[
          { value: 'always', label: tg.recapAlways },
          { value: 'longer', label: tg.recapLonger },
          { value: 'never', label: tg.recapNever },
        ]}
        value={recapMode}
        onChange={(mode: RecapMode) => setRecap(mode)}
      />
      {recapMode === 'longer' && (
        <div className={styles.field}>
          <label className={styles.label} htmlFor="recap-min">
            {tg.recapMinutes}
          </label>
          <select
            id="recap-min"
            className={styles.select}
            value={recapMinMinutes}
            onChange={(e) => setRecap('longer', Number(e.target.value))}
          >
            {[...new Set([5, 10, 15, 20, 30, 45, 60, recapMinMinutes])]
              .sort((a, b) => a - b)
              .map((n) => (
                <option key={n} value={n}>
                  {`${n} ${tg.minutesUnit}`}
                </option>
              ))}
          </select>
        </div>
      )}
    </section>
  );
}

function AppCard() {
  const client = useQueryClient();
  const settings = useQuery({
    queryKey: ['app-settings'],
    queryFn: () => commands.appSettingsGet(),
  });
  const save = useMutation({
    mutationFn: (s: AppSettings) => unwrap(commands.appSettingsSet(s)),
    onSettled: () => client.invalidateQueries({ queryKey: ['app-settings'] }),
  });
  const data = settings.data;
  if (!data) return null;
  const ta = t.app;
  const rows: { key: keyof AppSettings; label: string; hint: string }[] = [
    { key: 'closeToTray', label: ta.closeToTray, hint: ta.closeToTrayHint },
    { key: 'autostart', label: ta.autostart, hint: ta.autostartHint },
  ];
  return (
    <section className={styles.card} aria-labelledby="app-title">
      <h3 id="app-title" className="section-label">
        {ta.title}
      </h3>
      {rows.map((r) => (
        <div key={r.key} className={styles.cardHeader}>
          <div className={styles.cardTitle}>
            <span className={styles.label}>{r.label}</span>
            <span className={styles.muted}>{r.hint}</span>
          </div>
          <Switch
            label={r.label}
            checked={data[r.key]}
            disabled={save.isPending}
            onChange={(v) => save.mutate({ ...data, [r.key]: v })}
          />
        </div>
      ))}
      <p className={styles.muted}>{ta.trayHint}</p>
      {save.isError && (
        <p role="alert" className={styles.errorText}>
          {ta.saveError}
        </p>
      )}
    </section>
  );
}

const HIDDEN_QUERY = { view: { kind: 'hidden' }, text: null, sort: 'name' } as const;

/** Hidden games live here, out of the library panel. */
function HiddenGamesCard() {
  const list = useLibraryList(HIDDEN_QUERY);
  const bulk = useBulkUpdate();
  const th = t.hidden;
  const games = list.data ?? [];
  const unhide = (ids: string[]) => bulk.mutate({ ids, update: { hidden: false } });
  return (
    <section className={styles.card} aria-label={th.title}>
      {games.length === 0 ? (
        <p className={styles.muted}>{list.isLoading ? t.loading : th.empty}</p>
      ) : (
        <>
          <ul className={styles.hiddenList}>
            {games.map((g) => (
              <li key={g.id}>
                <span className={styles.hiddenThumb} aria-hidden="true">
                  <Cover title={g.title} cover={g.cover} header={g.header} />
                </span>
                <span className={styles.hiddenName}>{g.title}</span>
                <button
                  type="button"
                  className={buttons.secondary}
                  aria-label={th.unhideNamed(g.title)}
                  disabled={bulk.isPending}
                  onClick={() => unhide([g.id])}
                >
                  {th.unhide}
                </button>
              </li>
            ))}
          </ul>
          <div className={styles.hiddenActions}>
            <span className={styles.muted}>{th.count(games.length)}</span>
            <Link to="/library?view=hidden" className={buttons.secondary}>
              {th.openInLibrary}
            </Link>
            <button
              type="button"
              className={buttons.secondary}
              disabled={bulk.isPending}
              onClick={() => unhide(games.map((g) => g.id))}
            >
              {th.unhideAll}
            </button>
          </div>
        </>
      )}
    </section>
  );
}

function MetadataCard() {
  return (
    <section className={styles.card} aria-labelledby="metadata-title">
      <h3 id="metadata-title" className="section-label">
        {t.metadataTitle}
      </h3>
      <div className={styles.field}>
        <span className={styles.label} id="sgdb-label">
          {t.sgdb.title}
        </span>
        <span className={styles.muted}>{t.sgdb.provides}</span>
        <p className={styles.muted}>{t.sgdb.help}</p>
        <SecretKeyField kind="steamgriddb_api_key" labelId="sgdb-label" />
      </div>
      <div className={styles.field}>
        <span className={styles.label} id="igdb-label">
          {t.igdb.title}
        </span>
        <span className={styles.muted}>{t.igdb.provides}</span>
        <p className={styles.muted}>{t.igdb.help}</p>
        <span className={styles.label} id="igdb-id-label">
          {t.igdb.clientId}
        </span>
        <SecretKeyField kind="igdb_client_id" labelId="igdb-id-label" />
        <span className={styles.label} id="igdb-secret-label">
          {t.igdb.clientSecret}
        </span>
        <SecretKeyField kind="igdb_client_secret" labelId="igdb-secret-label" />
      </div>
    </section>
  );
}

function UpdateRow() {
  const { status, version, check, install } = useUpdates();
  const tu = fr.updates;
  const busy = status === 'checking' || status === 'downloading' || status === 'installing';
  const message =
    status === 'upToDate'
      ? tu.upToDate
      : status === 'error'
        ? tu.error
        : status === 'available' && version
          ? tu.available(version)
          : status === 'downloading' || status === 'installing'
            ? tu.installing
            : null;
  return (
    <div className={styles.updateRow}>
      {status === 'available' ? (
        <button type="button" className={buttons.primary} onClick={() => void install()}>
          {tu.install}
        </button>
      ) : (
        <button
          type="button"
          className={buttons.secondary}
          disabled={busy}
          onClick={() => void check()}
        >
          {status === 'checking' ? tu.checking : tu.check}
        </button>
      )}
      {message && (
        <span className={styles.muted} role="status">
          {message}
        </span>
      )}
      <a
        className={styles.inlineLink}
        href="https://github.com/Gen0miX/ignite/releases"
        target="_blank"
        rel="noreferrer"
      >
        {tu.releases}
      </a>
    </div>
  );
}

function AboutCard() {
  const info = useQuery({
    queryKey: ['app-info'],
    queryFn: () => commands.appInfo(),
    enabled: isTauri(),
    staleTime: Infinity,
  });

  let body;
  if (!isTauri() || info.isError) {
    body = <p className={styles.muted}>{t.backendUnavailable}</p>;
  } else if (!info.data) {
    body = <p className={styles.muted}>{t.loading}</p>;
  } else {
    body = (
      <dl className={styles.details}>
        <dt>{t.version}</dt>
        <dd>{info.data.version}</dd>
        <dt>{t.database}</dt>
        <dd className="mono">{info.data.dbPath}</dd>
        <dt>{t.dataDir}</dt>
        <dd className="mono">{info.data.dataDir}</dd>
      </dl>
    );
  }

  return (
    <section className={styles.card} aria-label={t.about}>
      {body}
      {isTauri() && <UpdateRow />}
    </section>
  );
}

/** Goldberg's global options (every game using it). */
function GoldbergCard() {
  return (
    <section className={styles.card} aria-labelledby="goldberg-title">
      <h3 id="goldberg-title" className="section-label">
        {fr.goldbergOptions.globalTitle}
      </h3>
      <GoldbergOptions />
    </section>
  );
}

/** Steam account and Web API key (achievements, playtime, store data). */
function SteamAccountCard() {
  const settings = useSteamSettings();
  if (!settings.data) return null;
  return (
    <section className={styles.card} aria-labelledby="steam-account-title">
      <header className={styles.cardHeader}>
        <SourceChip kind="steam" />
        <div className={styles.cardTitle}>
          <h3 id="steam-account-title">{t.steam.title}</h3>
          <span className={styles.muted}>{t.steam.accountsProvides}</span>
        </div>
      </header>
      <SteamAccount key={settings.data.steamId ?? 'none'} />
      <SteamApiKey present={settings.data.apiKeyPresent} />
    </section>
  );
}

function RetroAchievementsCard() {
  return (
    <section className={styles.card} aria-labelledby="ra-account-title">
      <h3 id="ra-account-title" className="section-label">
        RetroAchievements
      </h3>
      <RaAccount
        keyField={<SecretKeyField kind="retroachievements_api_key" labelId="ra-label" />}
      />
    </section>
  );
}

type SectionId =
  | 'general'
  | 'sources'
  | 'emulators'
  | 'achievements'
  | 'captures'
  | 'accounts'
  | 'hidden'
  | 'about';

const SECTIONS: { id: SectionId; icon: LucideIcon; backend: boolean }[] = [
  { id: 'general', icon: SlidersHorizontal, backend: false },
  { id: 'sources', icon: Library, backend: true },
  { id: 'emulators', icon: Gamepad2, backend: true },
  { id: 'achievements', icon: Trophy, backend: true },
  { id: 'captures', icon: Camera, backend: true },
  { id: 'accounts', icon: KeyRound, backend: true },
  { id: 'hidden', icon: EyeOff, backend: true },
  { id: 'about', icon: Info, backend: false },
];

function SourcesSection() {
  const sources = useSourcesStatus();
  return (
    <>
      {sources.data?.map((card) =>
        card.kind === 'steam' ? (
          <SteamCard key={card.kind} card={card} />
        ) : (
          <GenericSourceCard key={card.kind} card={card} />
        ),
      )}
      <p className={styles.muted}>{t.otherSources}</p>
    </>
  );
}

function SectionBody({ id }: { id: SectionId }) {
  switch (id) {
    case 'general':
      return (
        <>
          <GeneralCard />
          {isTauri() && <AppCard />}
        </>
      );
    case 'sources':
      return <SourcesSection />;
    case 'emulators':
      return <EmulatorsCard />;
    case 'achievements':
      return (
        <>
          <TrackingCard />
          <NotificationsCard />
          <GoldbergCard />
        </>
      );
    case 'captures':
      return <CaptureFoldersCard />;
    case 'accounts':
      return (
        <>
          <SteamAccountCard />
          <RetroAchievementsCard />
          <MetadataCard />
        </>
      );
    case 'hidden':
      return <HiddenGamesCard />;
    case 'about':
      return <AboutCard />;
  }
}

/** Réglages: categories on the left, one category at a time (`/settings/:section`). */
export function SettingsPage() {
  const { section } = useParams();
  const current = SECTIONS.find((s) => s.id === section);
  if (!current) return <Navigate to="/settings/general" replace />;
  const info = t.nav;
  return (
    <PageMain>
      <PageHeader title={t.sourcesTitle} />
      <div className={styles.layout}>
        <nav className={styles.nav} aria-label={t.sourcesTitle}>
          <ul>
            {SECTIONS.map(({ id, icon: Icon }) => (
              <li key={id}>
                <NavLink
                  to={`/settings/${id}`}
                  className={({ isActive }) =>
                    `${styles.navLink} ${isActive ? styles.navActive : ''}`
                  }
                >
                  <Icon size={16} aria-hidden="true" />
                  {info[id]}
                </NavLink>
              </li>
            ))}
          </ul>
        </nav>
        <div className={styles.content}>
          <header className={styles.sectionHeader}>
            <h2 className={styles.sectionTitle}>{info[current.id]}</h2>
            <p className={styles.muted}>{t.navIntro[current.id]}</p>
          </header>
          {current.backend && !isTauri() ? (
            <EmptyState>{t.backendUnavailable}</EmptyState>
          ) : (
            <SectionBody key={current.id} id={current.id} />
          )}
        </div>
      </div>
    </PageMain>
  );
}
