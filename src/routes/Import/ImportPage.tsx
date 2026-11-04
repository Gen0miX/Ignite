import { EmptyState } from '../../components/EmptyState/EmptyState';
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { fr } from '../../i18n/fr';

export function ImportPage() {
  return (
    <>
      <PageHeader title={fr.import.title} />
      <EmptyState>{fr.import.empty}</EmptyState>
    </>
  );
}
