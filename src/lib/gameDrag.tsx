import { useRef, type PointerEvent } from 'react';
import { create } from 'zustand';
import { strings } from '../i18n';

/**
 * Dragging games onto a collection of the sidebar. Pointer events rather
 * than HTML5 drag and drop: Tauri keeps native file drops (Add a game) for
 * itself on Windows, which disables HTML5 drag and drop in the page.
 */
interface DragState {
  drag: { ids: string[]; x: number; y: number; over: string | null } | null;
  onDrop: ((collectionId: string, ids: string[]) => void) | null;
}

export const useGameDrag = create<DragState>(() => ({ drag: null, onDrop: null }));

const THRESHOLD = 6;

/** Drop target under the pointer: an element with `data-drop-collection`. */
function targetAt(x: number, y: number): string | null {
  const el = document.elementFromPoint(x, y)?.closest<HTMLElement>('[data-drop-collection]');
  return el?.dataset.dropCollection ?? null;
}

/** Pointer handlers for a draggable card; `dragged()` tells a click to ignore itself. */
export function useCardDrag(getIds: () => string[]) {
  const start = useRef<{ x: number; y: number; id: number } | null>(null);
  const moved = useRef(false);

  const onPointerDown = (e: PointerEvent<HTMLElement>) => {
    if (e.button !== 0) return;
    start.current = { x: e.clientX, y: e.clientY, id: e.pointerId };
    moved.current = false;
  };

  const onPointerMove = (e: PointerEvent<HTMLElement>) => {
    const s = start.current;
    if (!s || s.id !== e.pointerId) return;
    if (!moved.current) {
      if (Math.hypot(e.clientX - s.x, e.clientY - s.y) < THRESHOLD) return;
      moved.current = true;
      e.currentTarget.setPointerCapture(e.pointerId);
    }
    useGameDrag.setState({
      drag: {
        ids: useGameDrag.getState().drag?.ids ?? getIds(),
        x: e.clientX,
        y: e.clientY,
        over: targetAt(e.clientX, e.clientY),
      },
    });
  };

  const end = (e: PointerEvent<HTMLElement>, drop: boolean) => {
    const { drag, onDrop } = useGameDrag.getState();
    if (drop && drag?.over && onDrop) onDrop(drag.over, drag.ids);
    if (moved.current && e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
    start.current = null;
    useGameDrag.setState({ drag: null });
  };

  return {
    handlers: {
      onPointerDown,
      onPointerMove,
      onPointerUp: (e: PointerEvent<HTMLElement>) => end(e, true),
      onPointerCancel: (e: PointerEvent<HTMLElement>) => end(e, false),
    },
    /** True right after a drag, so the click that follows is ignored. */
    dragged: () => {
      const was = moved.current;
      moved.current = false;
      return was;
    },
  };
}

/** Label following the pointer while games are dragged. */
export function DragGhost() {
  const drag = useGameDrag((s) => s.drag);
  if (!drag) return null;
  return (
    <div
      aria-hidden="true"
      style={{
        position: 'fixed',
        left: drag.x + 14,
        top: drag.y + 10,
        zIndex: 70,
        pointerEvents: 'none',
        padding: '4px 12px',
        borderRadius: 999,
        background: drag.over ? 'var(--color-accent)' : 'var(--color-active)',
        color: drag.over ? 'var(--color-on-accent)' : 'var(--color-text)',
        fontSize: 'var(--text-sm)',
        fontWeight: 700,
        boxShadow: 'var(--shadow-modal)',
      }}
    >
      {strings.selection.dragCount(drag.ids.length)}
    </div>
  );
}
