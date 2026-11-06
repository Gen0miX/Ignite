/** Gamepad input for couch mode (standard mapping, Xbox layout). */

export type PadAction =
  'left' | 'right' | 'up' | 'down' | 'a' | 'b' | 'x' | 'y' | 'lb' | 'rb' | 'menu';

/** Minimal shape of a browser `Gamepad`, so tests can build one. */
export interface PadState {
  buttons: ReadonlyArray<{ pressed: boolean }>;
  axes: ReadonlyArray<number>;
}

// https://w3c.github.io/gamepad/#remapping
const BUTTONS: [number, PadAction][] = [
  [0, 'a'],
  [1, 'b'],
  [2, 'x'],
  [3, 'y'],
  [4, 'lb'],
  [5, 'rb'],
  [9, 'menu'],
  [12, 'up'],
  [13, 'down'],
  [14, 'left'],
  [15, 'right'],
];
const DEADZONE = 0.5;
const DIRECTIONS: ReadonlySet<PadAction> = new Set(['left', 'right', 'up', 'down']);

/** Actions currently held on a pad (buttons, D-pad and left stick). */
export function heldActions(pad: PadState): Set<PadAction> {
  const held = new Set<PadAction>();
  for (const [i, action] of BUTTONS) if (pad.buttons[i]?.pressed) held.add(action);
  const [x = 0, y = 0] = pad.axes;
  if (x <= -DEADZONE) held.add('left');
  if (x >= DEADZONE) held.add('right');
  if (y <= -DEADZONE) held.add('up');
  if (y >= DEADZONE) held.add('down');
  return held;
}

/**
 * Turns held actions into discrete presses: every action fires when pressed,
 * directions then repeat while held (after `delay`, every `interval` ms).
 */
export class PadRepeater {
  private since = new Map<PadAction, { at: number; last: number }>();

  constructor(
    private readonly delay = 400,
    private readonly interval = 120,
  ) {}

  update(held: ReadonlySet<PadAction>, now: number): PadAction[] {
    const fired: PadAction[] = [];
    for (const action of held) {
      const s = this.since.get(action);
      if (!s) {
        this.since.set(action, { at: now, last: now });
        fired.push(action);
      } else if (
        DIRECTIONS.has(action) &&
        now - s.at >= this.delay &&
        now - s.last >= this.interval
      ) {
        s.last = now;
        fired.push(action);
      }
    }
    for (const action of [...this.since.keys()]) if (!held.has(action)) this.since.delete(action);
    return fired;
  }
}

/** Keyboard equivalents, so couch mode also works without a pad. */
export function keyAction(key: string): PadAction | null {
  switch (key) {
    case 'ArrowLeft':
      return 'left';
    case 'ArrowRight':
      return 'right';
    case 'ArrowUp':
      return 'up';
    case 'ArrowDown':
      return 'down';
    case 'Enter':
      return 'a';
    case 'Escape':
    case 'Backspace':
      return 'b';
    case 'r':
    case 'R':
      return 'x';
    case 'i':
    case 'I':
      return 'y';
    case 'PageUp':
    case 'q':
    case 'Q':
      return 'lb';
    case 'PageDown':
    case 'e':
    case 'E':
      return 'rb';
    default:
      return null;
  }
}

/** Index after moving by `delta`, clamped to the list. */
export function step(index: number, delta: number, length: number): number {
  if (length === 0) return 0;
  return Math.min(length - 1, Math.max(0, index + delta));
}

/** Index after moving by `delta`, wrapping around (shelves, moods). */
export function cycle(index: number, delta: number, length: number): number {
  if (length === 0) return 0;
  return (((index + delta) % length) + length) % length;
}
