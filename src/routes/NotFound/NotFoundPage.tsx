import { PageMain } from '../../app/AppShell';
import { Link } from 'react-router-dom';
import buttons from '../../components/Button/Button.module.css';
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { strings } from '../../i18n';

export function NotFoundPage() {
  return (
    <PageMain>
      <PageHeader title={strings.notFound.title} />
      <div>
        <Link to="/library" className={buttons.secondary}>
          {strings.notFound.action}
        </Link>
      </div>
    </PageMain>
  );
}
