import { PageMain } from '../../app/AppShell';
import { EmptyState } from '../../components/EmptyState/EmptyState';
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { fr } from '../../i18n/fr';

export function StatsPage() {
  return (
    <PageMain>
      <PageHeader title={fr.stats.title} />
      <EmptyState>{fr.stats.empty}</EmptyState>
    </PageMain>
  );
}
