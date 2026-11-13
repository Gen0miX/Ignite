import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RouterProvider, createMemoryRouter } from 'react-router-dom';
import { routes } from '../app/routes';

function renderAt(path: string) {
  const router = createMemoryRouter(routes, { initialEntries: [path] });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return router;
}

describe('app shell', () => {
  it('opens on the home page', async () => {
    renderAt('/');
    expect(await screen.findByRole('heading', { name: 'Accueil' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Accueil' })).toHaveAttribute('aria-current', 'page');
  });

  it('shows the library with the sidebar', async () => {
    renderAt('/library');
    expect(await screen.findByRole('heading', { level: 1, name: 'Tous les jeux' })).toBeVisible();
    expect(screen.getByRole('navigation', { name: 'Bibliothèque' })).toBeVisible();
    expect(screen.getByRole('link', { name: /Tous les jeux/ })).toHaveAttribute(
      'aria-current',
      'page',
    );
    expect(screen.getByRole('link', { name: /Installés/ })).not.toHaveAttribute('aria-current');
  });

  it('navigates between routes from the sidebar', async () => {
    const user = userEvent.setup();
    const router = renderAt('/library');

    await user.click(screen.getByRole('link', { name: /Statistiques/ }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Statistiques' })).toBeVisible();

    await user.click(screen.getByRole('link', { name: /Réglages/ }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Réglages' })).toBeVisible();
    expect(router.state.location.pathname).toBe('/settings/general');
    // The library panel only shows on library screens.
    expect(screen.queryByRole('link', { name: /Installés/ })).not.toBeInTheDocument();

    await user.click(screen.getByRole('link', { name: 'Bibliothèque' }));
    await user.click(await screen.findByRole('link', { name: /Installés/ }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Installés' })).toBeVisible();
    expect(screen.getByRole('link', { name: /Installés/ })).toHaveAttribute('aria-current', 'page');
  });

  it.each([
    ['/game/abc', 'Fiche du jeu'],
    ['/import', 'Importer des jeux'],
    ['/nope', 'Page introuvable'],
    ['/couch', 'Mode Canapé'],
  ])('renders %s', async (path, title) => {
    renderAt(path);
    expect(await screen.findByRole('heading', { level: 1, name: title })).toBeVisible();
  });

  it('shows that the backend is unavailable outside Tauri', async () => {
    renderAt('/settings/sources');
    expect((await screen.findAllByText(/Backend indisponible/))[0]).toBeVisible();
  });

  it('shows the settings one category at a time', async () => {
    const user = userEvent.setup();
    const router = renderAt('/settings/general');
    const nav = await screen.findByRole('navigation', { name: 'Réglages' });
    for (const name of ['Général', 'Sources', 'Succès et suivi', 'Comptes et clés', 'À propos'])
      expect(within(nav).getByRole('link', { name })).toBeVisible();
    expect(within(nav).getByRole('link', { name: 'Général' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    expect(screen.getByRole('heading', { level: 2, name: 'Général' })).toBeVisible();

    await user.click(within(nav).getByRole('link', { name: 'À propos' }));
    expect(router.state.location.pathname).toBe('/settings/about');
    expect(screen.getByRole('heading', { level: 2, name: 'À propos' })).toBeVisible();
  });

  it('sends an unknown settings page to Général', async () => {
    const router = renderAt('/settings/nope');
    await screen.findByRole('navigation', { name: 'Réglages' });
    expect(router.state.location.pathname).toBe('/settings/general');
  });
});
