import { useEffect, useLayoutEffect, useRef } from 'react';

interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

const DURATION = 280;
const EASING = 'cubic-bezier(0.2, 0.7, 0.2, 1)';

/** Card boxes relative to the row (offset*, so running transforms don't count). */
function measure(row: HTMLElement): Map<string, Box> {
  const boxes = new Map<string, Box>();
  for (const el of row.children) {
    if (!(el instanceof HTMLElement) || !el.dataset.gameId) continue;
    boxes.set(el.dataset.gameId, {
      x: el.offsetLeft,
      y: el.offsetTop,
      w: el.offsetWidth,
      h: el.offsetHeight,
    });
  }
  return boxes;
}

/** Moves each visible card from its old box to its new one (FLIP). */
function animate(row: HTMLElement, before: Map<string, Box>, after: Map<string, Box>) {
  const rowTop = row.getBoundingClientRect().top;
  const viewport = window.innerHeight;
  for (const el of row.children) {
    if (!(el instanceof HTMLElement) || typeof el.animate !== 'function') continue;
    const id = el.dataset.gameId;
    const a = id ? before.get(id) : undefined;
    const b = id ? after.get(id) : undefined;
    if (!a || !b || b.w === 0 || b.h === 0) continue;
    // Off-screen cards before and after: not worth animating.
    const visible = (box: Box) => rowTop + box.y < viewport && rowTop + box.y + box.h > 0;
    if (!visible(a) && !visible(b)) continue;
    const dx = a.x - b.x;
    const dy = a.y - b.y;
    const sx = a.w / b.w;
    const sy = a.h / b.h;
    if (Math.abs(dx) < 1 && Math.abs(dy) < 1 && Math.abs(sx - 1) < 0.01) continue;
    for (const running of el.getAnimations()) running.cancel();
    el.animate(
      [
        { transformOrigin: '0 0', transform: `translate(${dx}px, ${dy}px) scale(${sx}, ${sy})` },
        { transformOrigin: '0 0', transform: 'none' },
      ],
      { duration: DURATION, easing: EASING },
    );
  }
}

/**
 * Animates the cover grid when its width changes (side panel opened or closed,
 * menu reduced, window resized) or `layoutKey` does (cover size): cards glide
 * to their new place instead of jumping. `items` changing only refreshes the
 * remembered layout.
 */
export function useGridFlip<T extends HTMLElement>(items: unknown, layoutKey?: unknown) {
  const ref = useRef<T>(null);
  const last = useRef<{ boxes: Map<string, Box>; width: number }>({ boxes: new Map(), width: -1 });
  const lastKey = useRef(layoutKey);

  useLayoutEffect(() => {
    const row = ref.current;
    if (row) last.current = { boxes: measure(row), width: row.clientWidth };
  }, [items]);

  // Same width, other layout (cover size changed): glide as well.
  useLayoutEffect(() => {
    const row = ref.current;
    if (!row || lastKey.current === layoutKey) return;
    lastKey.current = layoutKey;
    const boxes = measure(row);
    const reduced = window.matchMedia?.('(prefers-reduced-motion: reduce)');
    if (!reduced?.matches) animate(row, last.current.boxes, boxes);
    last.current = { boxes, width: row.clientWidth };
  }, [layoutKey]);

  useEffect(() => {
    const row = ref.current;
    if (!row || typeof ResizeObserver === 'undefined') return;
    const reduced = window.matchMedia?.('(prefers-reduced-motion: reduce)');
    const observer = new ResizeObserver(() => {
      const boxes = measure(row);
      const width = row.clientWidth;
      const prev = last.current;
      if (prev.width >= 0 && width !== prev.width && !reduced?.matches) {
        animate(row, prev.boxes, boxes);
      }
      last.current = { boxes, width };
    });
    observer.observe(row);
    return () => observer.disconnect();
  }, []);

  return ref;
}
