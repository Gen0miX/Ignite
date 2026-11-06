import { PadRepeater, cycle, heldActions, keyAction, step, type PadState } from './gamepad';

function pad(pressed: number[], axes: number[] = [0, 0]): PadState {
  return {
    buttons: Array.from({ length: 17 }, (_, i) => ({ pressed: pressed.includes(i) })),
    axes,
  };
}

describe('gamepad', () => {
  it('maps standard buttons and the left stick', () => {
    expect([...heldActions(pad([0, 5]))].sort()).toEqual(['a', 'rb']);
    expect([...heldActions(pad([14]))]).toEqual(['left']);
    expect([...heldActions(pad([], [0.9, -0.8]))].sort()).toEqual(['right', 'up']);
    // Stick drift inside the dead zone is ignored.
    expect(heldActions(pad([], [0.2, -0.3])).size).toBe(0);
  });

  it('fires once per press, and repeats directions only', () => {
    const r = new PadRepeater(400, 100);
    expect(r.update(new Set(['a', 'right']), 0)).toEqual(['a', 'right']);
    expect(r.update(new Set(['a', 'right']), 200)).toEqual([]);
    expect(r.update(new Set(['a', 'right']), 450)).toEqual(['right']);
    expect(r.update(new Set(['a', 'right']), 500)).toEqual([]);
    expect(r.update(new Set(['a', 'right']), 560)).toEqual(['right']);
    // Released then pressed again: fires again.
    expect(r.update(new Set(), 600)).toEqual([]);
    expect(r.update(new Set(['a']), 610)).toEqual(['a']);
  });

  it('maps the keyboard', () => {
    expect(keyAction('Enter')).toBe('a');
    expect(keyAction('Escape')).toBe('b');
    expect(keyAction('PageDown')).toBe('rb');
    expect(keyAction('z')).toBeNull();
  });

  it('moves within lists', () => {
    expect(step(0, -1, 5)).toBe(0);
    expect(step(4, 1, 5)).toBe(4);
    expect(step(2, 1, 0)).toBe(0);
    expect(cycle(0, -1, 5)).toBe(4);
    expect(cycle(4, 1, 5)).toBe(0);
  });
});
