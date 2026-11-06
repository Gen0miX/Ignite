import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FLASH_MS, LAUNCH_DELAY_MS, TOAST_MS, launchWithUndo, toast, useToasts } from './toasts';

describe('toasts', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    useToasts.setState({ toasts: [], flashed: new Set() });
  });
  afterEach(() => vi.useRealTimers());

  it('shows a message, highlights the games, then clears both', () => {
    toast('2 jeux masqués', ['a', 'b']);
    expect(useToasts.getState().toasts.map((t) => t.message)).toEqual(['2 jeux masqués']);
    expect(useToasts.getState().flashed.has('a')).toBe(true);
    vi.advanceTimersByTime(FLASH_MS);
    expect(useToasts.getState().flashed.size).toBe(0);
    expect(useToasts.getState().toasts).toHaveLength(1);
    vi.advanceTimersByTime(TOAST_MS);
    expect(useToasts.getState().toasts).toHaveLength(0);
  });

  it('keeps at most three messages', () => {
    for (const m of ['1', '2', '3', '4']) toast(m);
    expect(useToasts.getState().toasts.map((t) => t.message)).toEqual(['2', '3', '4']);
  });

  it('launches after the delay unless cancelled', () => {
    const launch = vi.fn();
    launchWithUndo({ id: 'g', title: 'Elden Ring' }, launch);
    launchWithUndo({ id: 'g', title: 'Elden Ring' }, launch); // double request ignored
    expect(useToasts.getState().toasts).toHaveLength(1);
    expect(useToasts.getState().toasts[0]?.message).toBe('Lancement de Elden Ring…');
    vi.advanceTimersByTime(LAUNCH_DELAY_MS - 1);
    expect(launch).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(launch).toHaveBeenCalledTimes(1);
    expect(useToasts.getState().toasts).toHaveLength(0);
  });

  it('cancel stops the launch', () => {
    const launch = vi.fn();
    launchWithUndo({ id: 'h', title: 'Hades' }, launch);
    useToasts.getState().toasts[0]?.action?.run();
    vi.advanceTimersByTime(LAUNCH_DELAY_MS);
    expect(launch).not.toHaveBeenCalled();
  });
});
