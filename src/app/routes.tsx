import { Navigate, type RouteObject } from 'react-router-dom';
import { AppShell } from './AppShell';
import { HomePage } from '../routes/Home/HomePage';
import { LibraryPage } from '../routes/Library/LibraryPage';
import { GameDetailPage } from '../routes/GameDetail/GameDetailPage';
import { StatsPage } from '../routes/Stats/StatsPage';
import { SettingsSourcesPage } from '../routes/SettingsSources/SettingsSourcesPage';
import { ImportPage } from '../routes/Import/ImportPage';
import { NotFoundPage } from '../routes/NotFound/NotFoundPage';
import { CouchPage } from '../routes/Couch/CouchPage';

/** Route table, shared by the app router and the tests (memory router). */
export const routes: RouteObject[] = [
  // Fullscreen, outside the shell (no sidebar).
  { path: '/couch', element: <CouchPage /> },
  {
    path: '/',
    element: <AppShell />,
    children: [
      { index: true, element: <HomePage /> },
      { path: 'library', element: <LibraryPage /> },
      { path: 'game/:id', element: <GameDetailPage /> },
      { path: 'stats', element: <StatsPage /> },
      { path: 'settings', element: <Navigate to="/settings/sources" replace /> },
      { path: 'settings/sources', element: <SettingsSourcesPage /> },
      { path: 'import', element: <ImportPage /> },
      { path: '*', element: <NotFoundPage /> },
    ],
  },
];
