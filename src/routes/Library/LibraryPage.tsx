import { Link, useSearchParams } from 'react-router-dom';
import buttons from '../../components/Button/Button.module.css';
import { EmptyState } from '../../components/EmptyState/EmptyState';
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { fr } from '../../i18n/fr';

function titleForView(view: string | null): string {
  if (view === 'installed') return fr.library.installedTitle;
  if (view === 'favorites') return fr.library.favoritesTitle;
  if (view?.startsWith('status-')) {
    const status = view.slice('status-'.length);
    if (status in fr.status) return fr.status[status as keyof typeof fr.status];
  }
  return fr.library.title;
}

export function LibraryPage() {
  const [params] = useSearchParams();
  return (
    <>
      <PageHeader title={titleForView(params.get('view'))} meta={fr.library.count(0)} />
      <EmptyState
        title={fr.library.emptyTitle}
        action={
          <Link to="/import" className={buttons.primary}>
            {fr.library.emptyAction}
          </Link>
        }
      >
        {fr.library.emptyBody}
      </EmptyState>
    </>
  );
}
