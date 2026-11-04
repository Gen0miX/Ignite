import { Navigate, type RouteObject } from 'react-router-dom';
import { AppShell } from './AppShell';
import { LibraryPage } from '../routes/Library/LibraryPage';
import { GameDetailPage } from '../routes/GameDetail/GameDetailPage';
import { StatsPage } from '../routes/Stats/StatsPage';
import { SettingsSourcesPage } from '../routes/SettingsSources/SettingsSourcesPage';
import { ImportPage } from '../routes/Import/ImportPage';
import { NotFoundPage } from '../routes/NotFound/NotFoundPage';

/** Route table, shared by the app router and the tests (memory router). */
export const routes: RouteObject[] = [
  {
    path: '/',
    element: <AppShell />,
    children: [
      { index: true, element: <Navigate to="/library" replace /> },
      { path: 'library', element: <LibraryPage /> },
      { path: 'game/:id', element: <GameDetailPage /> },
      { path: 'stats', element: <StatsPage /> },
      { path: 'settings/sources', element: <SettingsSourcesPage /> },
      { path: 'import', element: <ImportPage /> },
      { path: '*', element: <NotFoundPage /> },
    ],
  },
];
