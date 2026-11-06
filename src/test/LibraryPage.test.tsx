import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RouterProvider, createMemoryRouter } from 'react-router-dom';
import type { GameDetail, GameSummary, LibraryCounts } from '../api/client';

const ok = <T,>(data: T) => Promise.resolve({ status: 'ok' as const, data });

const game = (id: string, title: string, extra: Partial<GameSummary> = {}): GameSummary => ({
  id,
  title,
  status: 'todo',
  favorite: false,
  hidden: false,
  installed: true,
  sources: ['steam'],
  playtimeMinutes: 0,
  lastPlayed: null,
  addedAt: '2026-01-01T00:00:00Z',
  cover: null,
  header: null,
  achievements: null,
  goals: null,
  ...extra,
});

const backend = vi.hoisted(() => ({
  games: [] as GameSummary[],
  counts: null as LibraryCounts | null,
  launched: [] as string[],
  syncCalls: 0,
  bulk: [] as string[][],
}));

vi.mock('@tauri-apps/api/event', () => ({ listen: () => Promise.resolve(() => {}) }));

vi.mock('../api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../api/client')>();
  const detail = (id: string): GameDetail => {
    const summary = backend.games.find((g) => g.id === id)!;
    return {
      summary,
      notes: null,
      preferredSourceId: null,
      hero: null,
      logo: null,
      tags: [],
      genres: [],
      modes: [],
      playSourceId: `src-${id}`,
      sources: [
        {
          id: `src-${id}`,
          kind: 'steam',
          externalId: '10',
          installed: true,
          installPath: 'C:\\Steam\\steamapps\\common\\X',
          sizeBytes: 1024,
          launchUri: 'steam://rungameid/10',
          playtimeMinutes: summary.playtimeMinutes,
          lastPlayed: null,
        },
      ],
      timeBySource: [],
    };
  };
  return {
    ...actual,
    isTauri: () => true,
    artworkSrc: () => undefined,
    commands: {
      ...actual.commands,
      libraryList: () => ok(backend.games),
      libraryCounts: () => ok(backend.counts!),
      gameGet: (id: string) => ok(detail(id)),
      launchGame: (id: string) => {
        backend.launched.push(id);
        return ok(null);
      },
      librarySync: () => {
        backend.syncCalls += 1;
        return ok([]);
      },
      sourcesStatus: () => ok([]),
      duplicatesList: () => ok([]),
      facets: () => ok({ genres: [], modes: [] }),
      collectionsList: () => ok([]),
      gamesBulkUpdate: (ids: string[], update: { status?: GameSummary['status'] }) => {
        // The changed games leave a filtered view, like in the real app.
        backend.games = backend.games.map((g) =>
          ids.includes(g.id) && update.status ? { ...g, status: update.status } : g,
        );
        backend.bulk.push(ids);
        return ok(null);
      },
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

const counts = (all: number): LibraryCounts => ({
  all,
  installed: all,
  favorites: 0,
  hidden: 0,
  status: { todo: all, playing: 0, done: 0, dropped: 0 },
  sources: all ? [{ kind: 'steam', count: all }] : [],
});

beforeEach(() => {
  backend.launched = [];
  backend.syncCalls = 0;
});

describe('library page', () => {
  it('works with no launcher at all: empty state with a sync action', async () => {
    backend.games = [];
    backend.counts = counts(0);
    const user = userEvent.setup();
    renderAt('/library');

    expect(await screen.findByText('Ta bibliothèque est vide')).toBeVisible();
    expect(screen.getByText('Aucune source connectée')).toBeVisible();
    await user.click(screen.getByRole('button', { name: 'Synchroniser' }));
    expect(backend.syncCalls).toBe(1);
  });

  it('shows games, selects one in the panel and launches with Enter', async () => {
    backend.games = [
      game('g1', 'Hades', { playtimeMinutes: 725 }),
      game('g2', 'Celeste', { installed: false }),
    ];
    backend.counts = counts(2);
    const user = userEvent.setup();
    const router = renderAt('/library');

    const grid = await screen.findByRole('grid', { name: 'Jeux' });
    const cards = within(grid).getAllByRole('gridcell');
    expect(cards).toHaveLength(2);
    expect(within(cards[0]!).getByText('12 h · Installé')).toBeVisible();

    await user.click(cards[1]!);
    expect(router.state.location.search).toContain('game=g2');
    const panel = await screen.findByRole('complementary', { name: 'Jeu sélectionné' });
    expect(within(panel).getByRole('heading', { name: 'Celeste' })).toBeVisible();
    expect(within(panel).getByRole('button', { name: /Jouer via Steam/ })).toBeEnabled();

    // Keyboard: arrow back to the first card, Enter launches it.
    cards[1]!.focus();
    await user.keyboard('{ArrowLeft}');
    expect(cards[0]).toHaveFocus();
    await user.keyboard('{Enter}');
    expect(backend.launched).toEqual(['g1']);

    // Sidebar shows the Steam source with its counter.
    expect(screen.getByRole('link', { name: /Steam/ })).toHaveTextContent('2');
  });

  it('confirms a status change with a toast and clears the selection', async () => {
    backend.games = [game('g1', 'Hades'), game('g2', 'Celeste'), game('g3', 'Tunic')];
    backend.counts = counts(3);
    const user = userEvent.setup();
    renderAt('/library');

    await screen.findByRole('grid', { name: 'Jeux' });
    await user.click(screen.getByRole('checkbox', { name: /Hades/ }));
    await user.click(screen.getByRole('checkbox', { name: /Celeste/ }));
    const bar = screen.getByRole('toolbar', { name: 'Sélection' });
    expect(within(bar).getByText('2 jeux sélectionnés')).toBeVisible();

    await user.selectOptions(within(bar).getByRole('combobox'), 'done');

    expect(await screen.findByText('2 jeux passés en « Terminé »')).toBeVisible();
    expect(backend.bulk).toEqual([['g1', 'g2']]);
    expect(screen.queryByRole('toolbar', { name: 'Sélection' })).not.toBeInTheDocument();
  });
});
