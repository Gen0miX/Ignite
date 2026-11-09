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
  overlay: false,
  overlayBuild: true,
  issues: ['notInstalled', 'needsAppid', 'noSaves'],
};

let currentStatus: GoldbergStatus = status;

const setup = vi.fn(() => ok({ ...status, installed: true, appid: 42 }));
const findSaves = vi.fn(() =>
  ok({ paths: ['C:\\Users\\Public\\Documents\\Steam\\42\\achievements.json'], unlocked: ['A'] }),
);
type Setting = { id: string; value: string | null; inherited: string };
const baseSettings: Setting[] = [
  { id: 'accountName', value: null, inherited: 'gse orca' },
  { id: 'language', value: null, inherited: 'english' },
  { id: 'ipCountry', value: null, inherited: 'US' },
  { id: 'overlay', value: null, inherited: '1' },
  { id: 'overlayNoProgress', value: null, inherited: '1' },
  { id: 'offline', value: '1', inherited: '0' },
];
let gameSettings: Setting[] = baseSettings;
const settingsView = () =>
  ok({ folder: 'C:\\Games\\Jeu local\\steam_settings', settings: gameSettings });
const setSetting = vi.fn((_gameId: string | null, id: string, value: string | null) => {
  gameSettings = gameSettings.map((x) => (x.id === id ? { ...x, value } : x));
  return settingsView();
});

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
          goldberg: currentStatus,
        }),
      goldbergStatus: () => ok(currentStatus),
      goldbergSetup: setup,
      goldbergFindSaves: findSaves,
      goldbergSettingsGet: () => settingsView(),
      goldbergSettingSet: setSetting,
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
  beforeEach(() => {
    currentStatus = status;
  });

  it('sends the Succès tab to the Goldberg tab until it is set up', async () => {
    const router = renderAt('/game/g1/succes');
    const nav = await screen.findByRole('navigation', { name: 'Sections de la fiche' });
    expect(within(nav).getByRole('link', { name: 'Succès' })).toBeVisible();
    expect(within(nav).getByRole('link', { name: 'Goldberg' })).toBeVisible();
    expect(
      await screen.findByText(/Ils s’afficheront ici une fois Goldberg installé et configuré/),
    ).toBeVisible();
    // No setup form on the Succès tab any more.
    expect(screen.queryByLabelText('AppID Steam')).not.toBeInTheDocument();
    await userEvent.setup().click(screen.getByRole('link', { name: /Configurer Goldberg/ }));
    expect(router.state.location.pathname).toBe('/game/g1/goldberg');
  });

  it('shows the setup card on the Goldberg tab', async () => {
    renderAt('/game/g1/goldberg');
    expect(await screen.findByText('Succès du jeu (Goldberg)')).toBeVisible();
    expect(screen.getByLabelText('AppID Steam')).toBeVisible();
    expect(screen.getByRole('button', { name: /Installer et configurer/ })).toBeVisible();
  });

  it('warns when the installed DLL has no overlay', async () => {
    currentStatus = {
      ...status,
      installed: true,
      appid: 42,
      definitionsReady: true,
      overlayBuild: false,
    };
    gameSettings = baseSettings;
    renderAt('/game/g1/goldberg');
    expect(await screen.findByText(/version « regular » de Goldberg/)).toBeVisible();
  });

  it('installs with the typed appid', async () => {
    const user = userEvent.setup();
    renderAt('/game/g1/goldberg');
    await user.type(await screen.findByLabelText('AppID Steam'), '42');
    await user.click(screen.getByRole('button', { name: /Installer et configurer/ }));
    expect(setup).toHaveBeenCalledWith('g1', 42, false);
  });

  it('shows a pending state while installing', async () => {
    setup.mockImplementationOnce(() => new Promise<never>(() => {}));
    const user = userEvent.setup();
    renderAt('/game/g1/goldberg');
    await user.type(await screen.findByLabelText('AppID Steam'), '42');
    await user.click(screen.getByRole('button', { name: /Installer et configurer/ }));
    expect(await screen.findByRole('button', { name: /Installation…/ })).toBeVisible();
  });

  it('finds and reports already-unlocked achievements', async () => {
    currentStatus = { ...status, installed: true, appid: 42, definitionsReady: true };
    const user = userEvent.setup();
    renderAt('/game/g1/goldberg');
    await user.click(await screen.findByRole('button', { name: /Chercher des succès existants/ }));
    expect(findSaves).toHaveBeenCalledWith('g1');
    expect(await screen.findByText('1 succès déjà débloqué trouvé dans 1 fichier.')).toBeVisible();
    expect(
      screen.getByText('C:\\Users\\Public\\Documents\\Steam\\42\\achievements.json'),
    ).toBeVisible();
  });

  it('shows the emulator options once installed, with inherited values', async () => {
    currentStatus = { ...status, installed: true, appid: 42, definitionsReady: true };
    gameSettings = baseSettings;
    renderAt('/game/g1/goldberg');
    expect(await screen.findByText('Options de l’émulateur')).toBeVisible();
    // Global overlay already on: the game inherits it.
    const overlay = await screen.findByRole('switch', { name: 'Activer l’overlay' });
    expect(overlay).toHaveAttribute('aria-checked', 'true');
    // Inverted option: disable_achievement_progress=1 shows as off.
    expect(screen.getByRole('switch', { name: 'Notifications de progression' })).toHaveAttribute(
      'aria-checked',
      'false',
    );
    expect(screen.getByLabelText('Langue du jeu')).toHaveValue('english');
    expect(screen.getAllByText('global').length).toBeGreaterThan(0);
  });

  it('overrides an option for the game and resets it', async () => {
    currentStatus = { ...status, installed: true, appid: 42, definitionsReady: true };
    gameSettings = baseSettings;
    const user = userEvent.setup();
    renderAt('/game/g1/goldberg');
    await user.selectOptions(await screen.findByLabelText('Langue du jeu'), 'french');
    expect(setSetting).toHaveBeenCalledWith('g1', 'language', 'french');

    await user.click(screen.getByRole('switch', { name: 'Notifications de progression' }));
    expect(setSetting).toHaveBeenCalledWith('g1', 'overlayNoProgress', '0');

    const name = screen.getByLabelText('Pseudo');
    await user.clear(name);
    await user.type(name, 'Gen0miX{Enter}');
    expect(setSetting).toHaveBeenCalledWith('g1', 'accountName', 'Gen0miX');

    // "offline" is set for this game: a reset button brings back the global value.
    await user.click(
      screen.getByRole('button', { name: 'Revenir à la valeur globale pour « Mode hors ligne »' }),
    );
    expect(setSetting).toHaveBeenCalledWith('g1', 'offline', null);
  });

  it('hides the emulator options until Goldberg is installed', async () => {
    renderAt('/game/g1/goldberg');
    expect(await screen.findByText('Succès du jeu (Goldberg)')).toBeVisible();
    expect(screen.queryByText('Options de l’émulateur')).not.toBeInTheDocument();
  });
});
