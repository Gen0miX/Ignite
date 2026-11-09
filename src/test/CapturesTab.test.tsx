import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RouterProvider, createMemoryRouter } from 'react-router-dom';
import type { CaptureItem, CaptureSource, GameDetail, GameSummary } from '../api/client';

const ok = <T,>(data: T) => Promise.resolve({ status: 'ok' as const, data });

const shot = (id: string, at: string, source: CaptureSource): CaptureItem => ({
  capture: {
    id,
    gameId: 'g1',
    source,
    path: String.raw`C:\Captures` + `\\${id}.png`,
    kind: 'image',
    takenAt: at,
    width: 2560,
    height: 1440,
    bytes: 431618,
    caption: null,
    thumbPath: null,
  },
  session: null,
});

const items = [
  shot('c3', '2025-10-31T11:00:00Z', 'gamebar'),
  shot('c2', '2025-10-29T10:30:00Z', 'steam'),
  shot('c1', '2025-10-29T10:22:00Z', 'steam'),
];

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
      exePath: null,
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
      capturesList: () => ok({ items, folders: [], scanning: false }),
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

describe('captures tab', () => {
  it('is reached by its URL, counts captures and filters by source', async () => {
    const user = userEvent.setup();
    renderAt('/game/g1/captures');
    const nav = await screen.findByRole('navigation', { name: 'Sections de la fiche' });
    expect(await within(nav).findByRole('link', { name: 'Captures 3' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    const filters = await screen.findByRole('group', { name: 'Source des captures' });
    expect(within(filters).getByRole('button', { name: 'Toutes · 3' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(screen.getAllByRole('button', { name: /^Capture de ENDLESS Space 2/ })).toHaveLength(3);

    await user.click(within(filters).getByRole('button', { name: 'Xbox Game Bar · 1' }));
    expect(screen.getAllByRole('button', { name: /^Capture de ENDLESS Space 2/ })).toHaveLength(1);
    expect(screen.getByText('Plus de captures ?')).toBeVisible();
  });

  it('opens the viewer, moves with the arrows and closes with Escape', async () => {
    const user = userEvent.setup();
    renderAt('/game/g1/captures');
    const tiles = await screen.findAllByRole('button', { name: /^Capture de ENDLESS Space 2/ });
    await user.click(tiles[1]!);

    const viewer = screen.getByRole('dialog', { name: 'Capture de ENDLESS Space 2' });
    expect(within(viewer).getByText(/^2 \/ 3/)).toBeVisible();
    expect(within(viewer).getByText('c2.png')).toBeVisible();
    expect(within(viewer).getByRole('button', { name: 'Fermer' })).toHaveFocus();

    await user.keyboard('{ArrowRight}');
    expect(within(viewer).getByText(/^3 \/ 3/)).toBeVisible();
    expect(within(viewer).getByRole('button', { name: 'Capture suivante' })).toBeDisabled();
    await user.keyboard('{ArrowRight}');
    expect(within(viewer).getByText(/^3 \/ 3/)).toBeVisible();

    await user.keyboard('{ArrowLeft}{ArrowLeft}');
    expect(within(viewer).getByText(/^1 \/ 3/)).toBeVisible();
    expect(within(viewer).getByRole('button', { name: 'Capture précédente' })).toBeDisabled();

    // Typing a caption does not move to another capture.
    await user.click(within(viewer).getByLabelText('Légende'));
    await user.keyboard('{ArrowRight}');
    expect(within(viewer).getByText(/^1 \/ 3/)).toBeVisible();

    await user.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(tiles[1]).toHaveFocus();
  });
});
