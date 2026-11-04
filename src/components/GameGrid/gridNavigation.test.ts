import { nextIndex } from './gridNavigation';

describe('nextIndex', () => {
  // 10 items, 4 columns:
  // 0 1 2 3
  // 4 5 6 7
  // 8 9
  it('moves horizontally within bounds', () => {
    expect(nextIndex('ArrowRight', 3, 10, 4)).toBe(4);
    expect(nextIndex('ArrowRight', 9, 10, 4)).toBe(9);
    expect(nextIndex('ArrowLeft', 0, 10, 4)).toBe(0);
  });

  it('moves vertically and stays put at the edges', () => {
    expect(nextIndex('ArrowDown', 1, 10, 4)).toBe(5);
    expect(nextIndex('ArrowDown', 6, 10, 4)).toBe(6); // nothing below
    expect(nextIndex('ArrowUp', 5, 10, 4)).toBe(1);
    expect(nextIndex('ArrowUp', 2, 10, 4)).toBe(2);
  });

  it('handles Home, End, unknown keys and empty grids', () => {
    expect(nextIndex('Home', 7, 10, 4)).toBe(0);
    expect(nextIndex('End', 0, 10, 4)).toBe(9);
    expect(nextIndex('a', 5, 10, 4)).toBe(5);
    expect(nextIndex('ArrowDown', 0, 0, 4)).toBe(-1);
  });
});
