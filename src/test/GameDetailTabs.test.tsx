import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RouterProvider, createMemoryRouter } from 'react-router-dom';
import type { GameDetail, GameSummary } from '../api/client';

const ok = <T,>(data: T) => Promise.resolve({ status: 'ok' as const, data });

const summary: GameSummary = {
  id: 'g1',
  title: 'ENDLESS Space 2',
  status: 'playing',
  favorite: false,
  hidden: false,
  installed: true,
  sources: ['steam'],
  playtimeMinutes: 8700,
  lastPlayed: null,
  addedAt: '2026-01-01T00:00:00Z',
  cover: null,
  header: null,
  achievements: { done: 17, total: 44 },
  goals: null,
};

const detail: GameDetail = {
  summary,
  notes: null,
  preferredSourceId: null,
  hero: null,
  logo: null,
  tags: [],
  genres: [],
  modes: [],
  playSourceId: 's1',
  sources: [
    {
      id: 's1',
      kind: 'steam',
      externalId: '392110',
      installed: true,
      installPath: 'C:\\Steam\\steamapps\\common\\ENDLESS Space 2',
      sizeBytes: null,
      launchUri: 'steam://rungameid/392110',
      playtimeMinutes: 8700,
      lastPlayed: null,
    },
  ],
  timeBySource: [],
  resumeNote: 'Victoire scientifique à 3 tours',
  resumeNoteAt: null,
};

vi.mock('@tauri-apps/api/event', () => ({ listen: () => Promise.resolve(() => {}) }));
vi.mock('../api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../api/client')>();
  return {
    ...actual,
    isTauri: () => true,
    artworkSrc: () => undefined,
    commands: {
      ...actual.commands,
      gameGet: () => ok(detail),
      sessionsList: () => ok([]),
      goalsList: () => ok([]),
      capturesList: () => ok({ items: [], folders: [], scanning: false }),
      capturesScan: () => Promise.resolve(null),
      gameNews: () => ok({ items: [], hidden: [] }),
      gameStoreMedia: () =>
        ok({
          steamAppid: null,
          byTitle: false,
          fromIgdb: false,
          igdbKeysMissing: false,
          media: { description: null, trailers: [], screenshots: [] },
        }),
      achievementsGet: () =>
        ok({ state: 'none', unlocked: 0, total: 0, items: [], syncedAt: null, error: null }),
      libraryCounts: () =>
        ok({
          all: 1,
          installed: 1,
          favorites: 0,
          hidden: 0,
          status: { todo: 0, playing: 1, done: 0, dropped: 0 },
          sources: [],
        }),
      collectionsList: () => ok([]),
      sourcesStatus: () => ok([]),
      trackerActive: () => Promise.resolve([]),
    },
  };
});

const { routes } = await import('../app/routes');

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

describe('game sheet tabs', () => {
  it('opens on Aperçu with the full header and the resume note', async () => {
    renderAt('/game/g1');
    const nav = await screen.findByRole('navigation', { name: 'Sections de la fiche' });
    expect(within(nav).getByRole('link', { name: 'Aperçu' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    expect(within(nav).getByRole('link', { name: /Succès 17\/44/ })).toBeVisible();
    expect(screen.getByText('Victoire scientifique à 3 tours')).toBeVisible();
    expect(screen.getByText('Temps total')).toBeVisible();
  });

  it('keeps the tab in the URL, with a compact header, and arrows move between tabs', async () => {
    const user = userEvent.setup();
    const router = renderAt('/game/g1/actus');
    const nav = await screen.findByRole('navigation', { name: 'Sections de la fiche' });
    const news = within(nav).getByRole('link', { name: 'Actus' });
    expect(news).toHaveAttribute('aria-current', 'page');
    expect(screen.getByText('145 h · En cours')).toBeVisible();
    expect(screen.queryByText('Temps total')).toBeNull();

    news.focus();
    await user.keyboard('{ArrowRight}');
    expect(router.state.location.pathname).toBe('/game/g1/lancement');
    expect(within(nav).getByRole('link', { name: 'Lancement' })).toHaveFocus();
    await user.keyboard('{ArrowRight}');
    expect(router.state.location.pathname).toBe('/game/g1');
  });

  it('an unknown tab falls back to Aperçu', async () => {
    const router = renderAt('/game/g1/nimporte');
    await screen.findByRole('navigation', { name: 'Sections de la fiche' });
    expect(router.state.location.pathname).toBe('/game/g1');
  });
});
