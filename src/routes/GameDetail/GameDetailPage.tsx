import { PageMain } from '../../app/AppShell';
import { Link } from 'react-router-dom';
import buttons from '../../components/Button/Button.module.css';
import { EmptyState } from '../../components/EmptyState/EmptyState';
import { PageHeader } from '../../components/PageHeader/PageHeader';
import { fr } from '../../i18n/fr';

export function GameDetailPage() {
  return (
    <PageMain>
      <PageHeader title={fr.game.title} />
      <EmptyState
        action={
          <Link to="/library" className={buttons.secondary}>
            {fr.game.backToLibrary}
          </Link>
        }
      >
        {fr.game.comingSoon}
      </EmptyState>
    </PageMain>
  );
}
