/** Index reached from `index` by a navigation key in a grid of `columns`. */
export function nextIndex(key: string, index: number, count: number, columns: number): number {
  if (count === 0) return -1;
  const cols = Math.max(1, columns);
  const clamp = (i: number) => Math.min(count - 1, Math.max(0, i));
  switch (key) {
    case 'ArrowRight':
      return clamp(index + 1);
    case 'ArrowLeft':
      return clamp(index - 1);
    case 'ArrowDown':
      return index + cols < count ? index + cols : index;
    case 'ArrowUp':
      return index - cols >= 0 ? index - cols : index;
    case 'Home':
      return 0;
    case 'End':
      return count - 1;
    default:
      return index;
  }
}

/** Number of items on the first row (same `offsetTop` as the first item). */
export function countColumns(items: HTMLElement[]): number {
  if (items.length === 0) return 1;
  const top = items[0]!.offsetTop;
  const n = items.findIndex((el) => el.offsetTop !== top);
  return n === -1 ? items.length : Math.max(1, n);
}
