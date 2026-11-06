import { useState, type FormEvent } from 'react';
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
  useClearSecret,
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
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { SourceChip } from '../../components/SourceBadge/SourceBadge';
import { Switch } from '../../components/Switch/Switch';
import { fr } from '../../i18n/fr';
import { formatLastPlayed } from '../../lib/format';
import styles from './SettingsSourcesPage.module.css';
import { EmulatorsCard } from './EmulatorsCard';

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
          <h2 id="source-steam">{t.steam.title}</h2>
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

      {settings.data && <SteamAccount key={settings.data.steamId ?? 'none'} />}
      {settings.data && <SteamApiKey present={settings.data.apiKeyPresent} />}

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
          <h2 id={`source-${card.kind}`}>{name}</h2>
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
      <h2 id="tracking-title" className="section-label">
        {t.trackingTitle}
      </h2>
      <dl className={styles.details}>
        <dt>{tt.method}</dt>
        <dd>{tt.methodValue}</dd>
        <dt>{tt.duplicates}</dt>
        <dd>{tt.duplicatesValue}</dd>
        <dt>{tt.history}</dt>
        <dd>{tt.historyValue}</dd>
      </dl>
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
      {save.isError && (
        <p role="alert" className={styles.errorText}>
          {tt.saveError}
        </p>
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
      <h2 id="app-title" className="section-label">
        {ta.title}
      </h2>
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

function MetadataCard() {
  return (
    <section className={styles.card} aria-labelledby="metadata-title">
      <h2 id="metadata-title" className="section-label">
        {t.metadataTitle}
      </h2>
      <div className={styles.field}>
        <span className={styles.label} id="sgdb-label">
          {t.sgdb.title}
        </span>
        <span className={styles.muted}>{t.sgdb.provides}</span>
        <p className={styles.muted}>{t.sgdb.help}</p>
        <SecretKeyField kind="steamgriddb_api_key" labelId="sgdb-label" />
      </div>
    </section>
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
    <section className={styles.card} aria-labelledby="settings-about">
      <h2 id="settings-about" className="section-label">
        {t.about}
      </h2>
      {body}
    </section>
  );
}

export function SettingsSourcesPage() {
  const sources = useSourcesStatus();
  return (
    <PageMain>
      <PageHeader title={t.sourcesTitle} />
      <nav className={styles.toc} aria-label={t.sourcesTitle}>
        <a href="#settings-sources">{t.sourcesSection}</a>
        <a href="#emulators-title">{t.emulators.title}</a>
        <a href="#tracking-title">{t.trackingTitle}</a>
        <a href="#app-title">{t.app.title}</a>
        <a href="#metadata-title">{t.metadataTitle}</a>
        <a href="#settings-about">{t.about}</a>
      </nav>
      <h2 id="settings-sources" className="section-label">
        {t.sourcesSection}
      </h2>
      <p className={styles.intro}>{t.optionalNote}</p>
      {isTauri() ? (
        sources.data?.map((card) =>
          card.kind === 'steam' ? (
            <SteamCard key={card.kind} card={card} />
          ) : (
            <GenericSourceCard key={card.kind} card={card} />
          ),
        )
      ) : (
        <EmptyState>{t.backendUnavailable}</EmptyState>
      )}
      <p className={styles.muted}>{t.otherSources}</p>
      {isTauri() && (
        <EmulatorsCard
          keyField={<SecretKeyField kind="retroachievements_api_key" labelId="ra-label" />}
        />
      )}
      {isTauri() && <TrackingCard />}
      {isTauri() && <AppCard />}
      {isTauri() && <MetadataCard />}
      <AboutCard />
    </PageMain>
  );
}
