import { render, screen } from '@testing-library/react';
import type { Popup } from '../api/bindings';
import { PopupCard, imageSrc, rarityLabel } from './PopupCard';
import { popupDuration, runQueue } from './queue';

const achievement: Popup = {
  kind: 'achievement',
  title: 'Premier sang',
  description: 'Vaincre le premier boss',
  game: 'Elden Ring',
  image: 'https://cdn.steamstatic.com/a.jpg',
  count: null,
  percent: null,
};

describe('PopupCard', () => {
  it('shows the achievement with its icon', () => {
    const { container } = render(<PopupCard popup={achievement} duration={6000} leaving={false} />);
    expect(screen.getByText('Succès débloqué')).toBeVisible();
    expect(screen.getByText('Premier sang')).toBeVisible();
    expect(screen.getByText('Vaincre le premier boss')).toBeVisible();
    expect(screen.getByText('Elden Ring')).toBeVisible();
    expect(container.querySelector('img')).toHaveAttribute('src', achievement.image);
  });

  it('sums up a burst of achievements', () => {
    render(
      <PopupCard
        popup={{ ...achievement, kind: 'achievements', title: 'Elden Ring', count: 5 }}
        duration={7000}
        leaving={false}
      />,
    );
    expect(screen.getByText('5 succès débloqués')).toBeVisible();
    // The game line is not repeated under a title that already is the game.
    expect(screen.getAllByText('Elden Ring')).toHaveLength(1);
  });

  it('falls back to an icon without image', () => {
    const { container } = render(
      <PopupCard
        popup={{ ...achievement, kind: 'session', image: null, description: '2 h 14' }}
        duration={6000}
        leaving={false}
      />,
    );
    expect(screen.getByText('Session terminée')).toBeVisible();
    expect(container.querySelector('img')).toBeNull();
  });
});

describe('rarity badge', () => {
  it('names rare and ultra-rare achievements', () => {
    expect(rarityLabel(2.14)).toBe('Ultra-rare · 2,1 %');
    expect(rarityLabel(12)).toBe('Rare · 12 %');
    expect(rarityLabel(48.5)).toBe('48,5 % des joueurs');
  });

  it('shows the share of players on the card', () => {
    render(<PopupCard popup={{ ...achievement, percent: 3.2 }} duration={6000} leaving={false} />);
    expect(screen.getByText('Ultra-rare · 3,2 %')).toBeVisible();
  });

  it('shows no badge without a percentage', () => {
    render(<PopupCard popup={achievement} duration={6000} leaving={false} />);
    expect(screen.queryByText(/des joueurs|rare ·/i)).toBeNull();
  });
});

describe('imageSrc', () => {
  it('keeps remote URLs and needs Tauri for local files', () => {
    expect(imageSrc('https://x/y.png')).toBe('https://x/y.png');
    expect(imageSrc('C:/img/a.jpg')).toBeUndefined();
    expect(imageSrc(null)).toBeUndefined();
  });
});

describe('runQueue', () => {
  it('shows every popup, then stops once the window closes', async () => {
    const queue: Popup[] = [achievement, { ...achievement, title: 'Deux' }];
    const shown: string[] = [];
    let idleCalls = 0;
    await runQueue({
      next: () => Promise.resolve(queue.shift() ?? null),
      idle: () => {
        idleCalls += 1;
        // A popup arrived while going idle: keep going once.
        if (idleCalls === 1) {
          queue.push({ ...achievement, title: 'Trois' });
          return Promise.resolve(true);
        }
        return Promise.resolve(false);
      },
      present: (p) => {
        shown.push(p.title);
        return Promise.resolve();
      },
    });
    expect(shown).toEqual(['Premier sang', 'Deux', 'Trois']);
    expect(idleCalls).toBe(2);
  });

  it('keeps summaries a bit longer', () => {
    expect(popupDuration({ kind: 'achievements' })).toBeGreaterThan(
      popupDuration({ kind: 'achievement' }),
    );
  });
});
