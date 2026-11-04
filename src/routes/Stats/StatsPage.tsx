import { EmptyState } from '../../components/EmptyState/EmptyState';
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { fr } from '../../i18n/fr';

export function StatsPage() {
  return (
    <>
      <PageHeader title={fr.stats.title} />
      <EmptyState>{fr.stats.empty}</EmptyState>
    </>
  );
}
