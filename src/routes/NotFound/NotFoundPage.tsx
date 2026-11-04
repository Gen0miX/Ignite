import { PageMain } from '../../app/AppShell';
import { Link } from 'react-router-dom';
import buttons from '../../components/Button/Button.module.css';
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { fr } from '../../i18n/fr';

export function NotFoundPage() {
  return (
    <PageMain>
      <PageHeader title={fr.notFound.title} />
      <div>
        <Link to="/library" className={buttons.secondary}>
          {fr.notFound.action}
        </Link>
      </div>
    </PageMain>
  );
}
