import { PageMain } from '../../app/AppShell';
import { EmptyState } from '../../components/EmptyState/EmptyState';
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { fr } from '../../i18n/fr';

export function ImportPage() {
  return (
    <PageMain>
      <PageHeader title={fr.import.title} />
      <EmptyState>{fr.import.empty}</EmptyState>
    </PageMain>
  );
}
