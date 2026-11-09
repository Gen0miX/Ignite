import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import type { SessionRecap } from '../api/client';

const ok = <T,>(data: T) => Promise.resolve({ status: 'ok' as const, data });

const backend = vi.hoisted(() => ({
  handlers: new Map<string, (e: { payload: unknown }) => void>(),
  saved: [] as unknown[],
  revealed: 0,
  notified: [] as string[],
  active: [] as unknown[],
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: (event: string, handler: (e: { payload: unknown }) => void) => {
    backend.handlers.set(event, handler);
    return Promise.resolve(() => backend.handlers.delete(event));
  },
}));

const recap: SessionRecap = {
  session: {
    id: 's1',
    gameId: 'g1',
    gameSourceId: null,
    startedAt: '2026-11-06T18:12:00Z',
    endedAt: '2026-11-06T20:36:00Z',
    durationSeconds: 8040,
    idleSeconds: 480,
    origin: 'auto',
    note: null,
    rating: null,
  },
  game: { id: 'g1', title: 'Satisfactory', logo: null, hero: null, header: null },
  activeSeconds: 8040,
  idlePeriods: [],
  timeline: [
    { start: '2026-11-06T18:12:00Z', end: '2026-11-06T19:04:00Z', idle: false },
    { start: '2026-11-06T19:04:00Z', end: '2026-11-06T19:12:00Z', idle: true },
    { start: '2026-11-06T19:12:00Z', end: '2026-11-06T20:36:00Z', idle: false },
  ],
  averageSeconds: 5640,
  totalMinutes: 3900,
  totalApprox: false,
  week: { minutes: 400, sessions: 3 },
  streak: { current: 3, best: 9 },
  achievements: null,
  openGoals: [],
  openGoalsTotal: 0,
  resumeNote: null,
  rating: null,
  status: 'todo',
  captures: [],
};

vi.mock('../api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../api/client')>();
  return {
    ...actual,
    isTauri: () => true,
    artworkSrc: () => undefined,
    commands: {
      ...actual.commands,
      sessionRecap: () => ok(recap),
      trackerActive: () => Promise.resolve(backend.active),
      windowReveal: () => {
        backend.revealed += 1;
        return Promise.resolve(null);
      },
      notify: (gameId: string, duration: string) => {
        backend.notified.push(`${gameId} · ${duration}`);
        return ok(null);
      },
      sessionRecapSave: (_id: string, input: unknown) => {
        backend.saved.push(input);
        return ok(null);
      },
    },
  };
});

const { useRecapEvents } = await import('../components/SessionRecap/useRecapEvents');
const { SessionRecapHost } = await import('../components/SessionRecap/SessionRecapHost');
const { useRecapQueue } = await import('../components/SessionRecap/recapQueue');
const { usePrefs } = await import('../lib/prefs');

let couch = false;
function Events() {
  useRecapEvents(isCouch);
  return null;
}
const isCouch = () => couch;

function renderApp() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <Events />
        <SessionRecapHost />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

async function sessionEnds() {
  await act(async () => {
    backend.handlers.get('session-recap-ready')!({
      payload: { sessionId: 's1', gameId: 'g1' },
    });
  });
}

beforeEach(() => {
  couch = false;
  backend.saved = [];
  backend.revealed = 0;
  backend.notified = [];
  backend.active = [];
  useRecapQueue.setState({ queue: [], pending: [] });
  usePrefs.setState({ recapMode: 'longer', recapMinMinutes: 15 });
});

describe('session recap', () => {
  it('opens on the event, in front, with the resume note focused', async () => {
    renderApp();
    await sessionEnds();
    const dialog = await screen.findByRole('dialog');
    expect(backend.revealed).toBe(1);
    expect(within(dialog).getByText('2 h 14')).toBeVisible();
    expect(within(dialog).getByText('+40 min par rapport à ta session moyenne')).toBeVisible();
    expect(within(dialog).getByLabelText('Où t’es-tu arrêté ?')).toHaveFocus();
    // "À faire" was the status: "En cours" is proposed.
    expect(within(dialog).getByRole('radio', { name: 'En cours' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
  });

  it('only notifies in couch mode, and opens when the user is back', async () => {
    couch = true;
    renderApp();
    await sessionEnds();
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(backend.notified).toEqual(['g1 · 2 h 14']);
    await act(async () => {
      window.dispatchEvent(new Event('focus'));
    });
    expect(await screen.findByRole('dialog')).toBeVisible();
  });

  it('Escape closes, Ctrl+Enter saves', async () => {
    const user = userEvent.setup();
    renderApp();
    await sessionEnds();
    await screen.findByRole('dialog');
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).toBeNull();

    await sessionEnds();
    const dialog = await screen.findByRole('dialog');
    await user.type(within(dialog).getByLabelText('Où t’es-tu arrêté ?'), 'Le train nord');
    await user.keyboard('{Control>}{Enter}{/Control}');
    expect(backend.saved).toEqual([{ resumeNote: 'Le train nord', rating: 0, status: 'playing' }]);
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('flames have a single checked value and move with the arrows', async () => {
    const user = userEvent.setup();
    renderApp();
    await sessionEnds();
    const group = await screen.findByRole('radiogroup', { name: 'Cette session' });
    const flames = within(group).getAllByRole('radio');
    await user.click(flames[3]!);
    await user.keyboard('{ArrowRight}');
    const checked = flames.filter((f) => f.getAttribute('aria-checked') === 'true');
    expect(checked).toHaveLength(1);
    expect(checked[0]).toHaveAccessibleName('Excellente (5 flammes)');
  });

  it('the "under 15 min" box writes the setting', async () => {
    const user = userEvent.setup();
    renderApp();
    await sessionEnds();
    const box = await screen.findByRole('checkbox', {
      name: 'Ne plus afficher pour les sessions de moins de 15 min',
    });
    expect(box).toBeChecked();
    await user.click(box);
    expect(usePrefs.getState().recapMode).toBe('always');
    await user.click(box);
    expect(usePrefs.getState()).toMatchObject({ recapMode: 'longer', recapMinMinutes: 15 });
  });
});
