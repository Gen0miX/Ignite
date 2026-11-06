import { QueryClientProvider } from '@tanstack/react-query';
import { RouterProvider, createBrowserRouter } from 'react-router-dom';
import { useRecapEvents } from '../components/SessionRecap/useRecapEvents';
import { queryClient } from './queryClient';
import { routes } from './routes';

const router = createBrowserRouter(routes);
const isCouch = () => router.state.location.pathname.startsWith('/couch');

/** Mounted above the router: recaps are caught even in couch mode. */
function RecapEvents() {
  useRecapEvents(isCouch);
  return null;
}

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <RecapEvents />
      <RouterProvider router={router} />
    </QueryClientProvider>
  );
}
