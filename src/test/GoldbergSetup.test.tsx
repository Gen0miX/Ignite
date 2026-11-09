import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { RouterProvider, createMemoryRouter } from 'react-router-dom';
import type { GameDetail, GameSummary, GoldbergStatus } from '../api/client';

const ok = <T,>(data: T) => Promise.resolve({ status: 'ok' as const, data });

const summary: GameSummary = {
  id: 'g1',
  title: 'Jeu local',
  status: 'playing',
  favorite: false,
  hidden: false,
  installed: true,
  sources: ['local'],
  playtimeMinutes: 60,
  lastPlayed: null,
  addedAt: '2026-01-01T00:00:00Z',
  cover: null,
  header: null,
  achievements: null,
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
      kind: 'local',
      externalId: 'C:\\Games\\Jeu local\\game.exe',
      installed: true,
      installPath: 'C:\\Games\\Jeu local',
      exePath: 'C:\\Games\\Jeu local\\game.exe',
      sizeBytes: null,
      launchUri: null,
      playtimeMinutes: 60,
      lastPlayed: null,
    },
  ],
  timeBySource: [],
  resumeNote: null,
  resumeNoteAt: null,
};

const status: GoldbergStatus = {
  gameId: 'g1',
  sourceId: 's1',
  gameTitle: 'Jeu local',
  exePath: 'C:\\Games\\Jeu local\\game.exe',
  gameDir: 'C:\\Games\\Jeu local',
  dllPath: 'C:\\Games\\Jeu local',
  architecture: 'x64',
  installed: false,
  originalBackedUp: false,
  appid: null,
  settingsDir: 'C:\\Games\\Jeu local\\steam_settings',
  definitionsReady: false,
  savesPath: null,
  savesReady: false,
  issues: ['notInstalled', 'needsAppid', 'noSaves'],
};

const setup = vi.fn(() => ok({ ...status, installed: true, appid: 42 }));

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
        ok({
          state: 'needsSetup',
          unlocked: 0,
          total: 0,
          items: [],
          syncedAt: null,
          error: null,
          goldberg: status,
        }),
      goldbergStatus: () => ok(status),
      goldbergSetup: setup,
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

describe('Goldberg setup', () => {
  it('shows the Succès tab and the setup card for a local game', async () => {
    renderAt('/game/g1/succes');
    const nav = await screen.findByRole('navigation', { name: 'Sections de la fiche' });
    expect(within(nav).getByRole('link', { name: 'Succès' })).toBeVisible();
    expect(await screen.findByText('Succès du jeu (Goldberg)')).toBeVisible();
    expect(screen.getByText('Installe Goldberg pour suivre les succès de ce jeu.')).toBeVisible();
    expect(screen.getByLabelText('AppID Steam')).toBeVisible();
    expect(screen.getByRole('button', { name: /Installer et configurer/ })).toBeVisible();
  });

  it('installs with the typed appid', async () => {
    const user = userEvent.setup();
    renderAt('/game/g1/succes');
    await user.type(await screen.findByLabelText('AppID Steam'), '42');
    await user.click(screen.getByRole('button', { name: /Installer et configurer/ }));
    expect(setup).toHaveBeenCalledWith('g1', 42, false);
  });
});
