import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { AchievementItem } from '../api/client';

const ok = <T,>(data: T) => Promise.resolve({ status: 'ok' as const, data });

const item = (apiName: string, extra: Partial<AchievementItem>): AchievementItem => ({
  apiName,
  name: apiName,
  description: null,
  unlockedAt: '2026-11-01T20:00:00Z',
  globalPercent: 40,
  icon: null,
  iconPending: false,
  hidden: false,
  ...extra,
});

const items = [
  item('Téléchargement', { iconPending: true }),
  item('Échec', {}),
  item('Prêt', { icon: 'C:\\cache\\a.jpg' }),
  item('Verrouillé', { unlockedAt: null, iconPending: false }),
];

vi.mock('../api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../api/client')>();
  return {
    ...actual,
    isTauri: () => true,
    artworkSrc: (p: string | null) => (p ? `asset://${p}` : undefined),
    commands: {
      ...actual.commands,
      achievementsGet: () =>
        ok({ state: 'ok', unlocked: 3, total: 4, items, syncedAt: null, error: null }),
    },
  };
});

const { AchievementsSection } = await import('../routes/GameDetail/AchievementsSection');

describe('achievement icons', () => {
  it('pulse while downloading, then show the image, a trophy or a padlock', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={client}>
        <AchievementsSection gameId="g1" sourceLabel="Steam" isEmulated={false} />
      </QueryClientProvider>,
    );
    // "Tous" view, so every achievement is listed.
    await userEvent.click(await screen.findByRole('radio', { name: /^Tous/ }));

    // The name can also show in the summary ("Ton plus rare"): take the list row.
    const box = async (name: string) =>
      (await screen.findAllByText(name))
        .map((el) => el.closest('li'))
        .find((li) => li !== null)!
        .querySelector('[aria-hidden="true"]')!;
    expect(await box('Téléchargement')).toHaveClass('img-pending');
    expect((await box('Téléchargement')).querySelector('svg')).toBeNull();
    expect(await box('Échec')).not.toHaveClass('img-pending');
    expect((await box('Échec')).querySelector('svg')).not.toBeNull();
    expect((await box('Prêt')).querySelector('img')).toHaveAttribute(
      'src',
      'asset://C:\\cache\\a.jpg',
    );
    expect((await box('Verrouillé')).querySelector('img')).toBeNull();
  });
});
