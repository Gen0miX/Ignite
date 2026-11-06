import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FLASH_MS, TOAST_MS, toast, useToasts } from './toasts';

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
});
