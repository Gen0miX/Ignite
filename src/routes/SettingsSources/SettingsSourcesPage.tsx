import { useQuery } from '@tanstack/react-query';
import { commands, isTauri } from '../../api/client';
import { EmptyState } from '../../components/EmptyState/EmptyState';
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { fr } from '../../i18n/fr';
import styles from './SettingsSourcesPage.module.css';

function AboutCard() {
  const info = useQuery({
    queryKey: ['app-info'],
    queryFn: () => commands.appInfo(),
    enabled: isTauri(),
    staleTime: Infinity,
  });

  let body;
  if (!isTauri() || info.isError) {
    body = <p className={styles.muted}>{fr.settings.backendUnavailable}</p>;
  } else if (!info.data) {
    body = <p className={styles.muted}>{fr.settings.loading}</p>;
  } else {
    body = (
      <dl className={styles.details}>
        <dt>{fr.settings.version}</dt>
        <dd>{info.data.version}</dd>
        <dt>{fr.settings.database}</dt>
        <dd>{info.data.dbPath}</dd>
        <dt>{fr.settings.dataDir}</dt>
        <dd>{info.data.dataDir}</dd>
      </dl>
    );
  }

  return (
    <section className={styles.card} aria-labelledby="settings-about">
      <h2 id="settings-about" className="section-label">
        {fr.settings.about}
      </h2>
      {body}
    </section>
  );
}

export function SettingsSourcesPage() {
  return (
    <>
      <PageHeader title={fr.settings.sourcesTitle} />
      <EmptyState>{fr.settings.sourcesEmpty}</EmptyState>
      <AboutCard />
    </>
  );
}
