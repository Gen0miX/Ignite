import { useState, type FormEvent } from 'react';
import { useQuery } from '@tanstack/react-query';
import { commands, isTauri, type SourceCard, type SourceStatus } from '../../api/client';
import {
  useClearSecret,
  useSetSecret,
  useSetSourceEnabled,
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
      <AboutCard />
    </PageMain>
  );
}
