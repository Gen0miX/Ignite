import { beforeEach, describe, expect, it } from 'vitest';
import { recapDecision, useRecapQueue } from './recapQueue';

const base = {
  mode: 'longer' as const,
  minMinutes: 15,
  activeSeconds: 3600,
  couch: false,
  otherGameRunning: false,
};

describe('recapDecision', () => {
  it('follows the end-of-session setting', () => {
    expect(recapDecision(base)).toBe('open');
    expect(recapDecision({ ...base, activeSeconds: 10 * 60 })).toBe('skip');
    expect(recapDecision({ ...base, mode: 'always', activeSeconds: 10 * 60 })).toBe('open');
    expect(recapDecision({ ...base, mode: 'never' })).toBe('skip');
  });

  it('notifies instead of opening in couch mode or while another game runs', () => {
    expect(recapDecision({ ...base, couch: true })).toBe('notify');
    expect(recapDecision({ ...base, otherGameRunning: true })).toBe('notify');
  });
});

describe('recap queue', () => {
  beforeEach(() => useRecapQueue.setState({ queue: [], pending: [] }));

  it('shows one recap at a time, in order', () => {
    const q = useRecapQueue.getState();
    q.open('a');
    q.open('b');
    q.open('a');
    expect(useRecapQueue.getState().queue).toEqual(['a', 'b']);
    useRecapQueue.getState().done('a');
    expect(useRecapQueue.getState().queue).toEqual(['b']);
  });

  it('keeps notified recaps until the user is back', () => {
    const q = useRecapQueue.getState();
    q.defer('c');
    expect(useRecapQueue.getState().queue).toEqual([]);
    useRecapQueue.getState().promote();
    expect(useRecapQueue.getState()).toMatchObject({ queue: ['c'], pending: [] });
  });
});
