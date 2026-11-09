import { useState } from 'react';
import { CheckCircle2, Download, FolderOpen, Info, RefreshCw, Search, XCircle } from 'lucide-react';
import {
  useGoldbergClearAppid,
  useGoldbergFindSaves,
  useGoldbergOpenSaves,
  useGoldbergSetup,
  useGoldbergStatus,
} from '../../api/hooks';
import buttons from '../../components/Button/Button.module.css';
import { GoldbergOptions } from '../../components/GoldbergOptions/GoldbergOptions';
import { Skeleton } from '../../components/Skeleton/Skeleton';
import { fr } from '../../i18n/fr';
import styles from './AchievementsTab.module.css';
import sheet from './GameDetailPage.module.css';

const t = fr.game;

/**
 * Goldberg tab of an executable: installation, then the emulator options
 * (overriding the global ones from Réglages › Sources).
 */
export function GoldbergTab({ gameId }: { gameId: string }) {
  const status = useGoldbergStatus(gameId);
  return (
    <section className={styles.tab} aria-labelledby="sheet-goldberg">
      <h2 id="sheet-goldberg" className="visually-hidden">
        {t.tabs.goldberg}
      </h2>
      {status.isPending ? (
        <Skeleton height={180} radius="var(--radius-xl)" />
      ) : status.isError ? (
        <p className={sheet.error}>{t.goldberg.error}</p>
      ) : (
        <GoldbergSetup gameId={gameId} />
      )}
    </section>
  );
}

/** Goldberg setup and installation check for a local game. */
function GoldbergSetup({ gameId }: { gameId: string }) {
  const status = useGoldbergStatus(gameId);
  const setup = useGoldbergSetup(gameId);
  const clear = useGoldbergClearAppid(gameId);
  const openSaves = useGoldbergOpenSaves(gameId);
  const findSaves = useGoldbergFindSaves(gameId);
  const [typed, setTyped] = useState<string | null>(null);
  const s = status.data;
  const appid = typed ?? (s?.appid ? String(s.appid) : '');

  if (!s) return null;
  const ready = s.installed && s.appid != null && s.definitionsReady;
  const valid = /^\d+$/.test(appid.trim()) && Number(appid) > 0;
  const issues = s.issues.filter((i) => !(ready && (i === 'noSaves' || i === 'needsAppid')));
  const busy = setup.isPending;

  const small = `${buttons.secondary} ${styles.small}`;

  return (
    <>
      <section className={`${sheet.card} ${styles.setup}`} aria-labelledby="goldberg-title">
        <div className={sheet.sectionHeader}>
          <h3 id="goldberg-title" className="section-label">
            {t.goldberg.title}
          </h3>
          <span className={ready ? styles.ready : styles.notReady}>
            {ready ? (
              <CheckCircle2 size={14} aria-hidden="true" />
            ) : (
              <XCircle size={14} aria-hidden="true" />
            )}
            {ready
              ? t.goldberg.readyShort
              : s.installed
                ? t.goldberg.installedShort
                : t.goldberg.notInstalled}
          </span>
        </div>
        {!ready && <p className={sheet.muted}>{t.goldberg.intro}</p>}

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
        {s.appid != null && !s.savesReady && (
          <p className={styles.note}>{t.goldberg.savesMissing}</p>
        )}
        {issues.includes('needsSteamKey') && <p className={styles.note}>{t.goldberg.noKey}</p>}

        <form
          className={styles.setupForm}
          onSubmit={(e) => {
            e.preventDefault();
            if (valid) setup.mutate({ appid: Number(appid), force: s.installed });
          }}
        >
          <label className={styles.appidLabel} htmlFor="goldberg-appid">
            {t.goldberg.appid}
          </label>
          <input
            id="goldberg-appid"
            className={`${sheet.input} ${styles.appidInput}`}
            inputMode="numeric"
            placeholder={t.goldberg.appidPlaceholder}
            value={appid}
            onChange={(e) => setTyped(e.target.value)}
            title={t.goldberg.appidHint}
          />
          <button
            type="submit"
            className={s.installed ? small : `${buttons.primary} ${styles.small}`}
            disabled={!valid || setup.isPending}
          >
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
            className={`${small} ${styles.iconOnly}`}
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
        </form>

        {s.appid != null && (
          <div className={styles.toolbar}>
            <button
              type="button"
              className={small}
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
              className={small}
              onClick={() => openSaves.mutate()}
              disabled={busy}
            >
              <FolderOpen size={14} aria-hidden="true" />
              {t.goldberg.openSaves}
            </button>
            <button
              type="button"
              className={`${small} ${styles.quiet}`}
              onClick={() => {
                clear.mutate();
                setTyped('');
              }}
              disabled={busy}
            >
              {t.goldberg.clearAppid}
            </button>
          </div>
        )}

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

        <details className={styles.techDetails}>
          <summary>{t.goldberg.details}</summary>
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
          {s.installed && (
            <p className={styles.note}>
              {s.originalBackedUp ? t.goldberg.backup : t.goldberg.noBackup}
            </p>
          )}
        </details>

        {(setup.isError || clear.isError || findSaves.isError) && (
          <p role="alert" className={sheet.error}>
            {t.goldberg.error}
          </p>
        )}
      </section>
      {s.installed && (
        <section
          className={`${sheet.card} ${styles.setup}`}
          aria-labelledby="goldberg-options-title"
        >
          <h3 id="goldberg-options-title" className="section-label">
            {fr.goldbergOptions.title}
          </h3>
          {!s.overlayBuild && (
            <p className={styles.overlayBuild} role="note">
              <Info size={14} aria-hidden="true" />
              {t.goldberg.noOverlayBuild}
            </p>
          )}
          <GoldbergOptions gameId={gameId} />
        </section>
      )}
    </>
  );
}
