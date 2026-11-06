import { useEffect, useRef, useState } from 'react';
import { PadRepeater, heldActions, keyAction, type PadAction } from './gamepad';

function pads(): Gamepad[] {
  if (typeof navigator === 'undefined' || !navigator.getGamepads) return [];
  return navigator.getGamepads().filter((p): p is Gamepad => p !== null);
}

/**
 * Calls `onAction` for gamepad presses (polled every frame while a pad is
 * connected) and, when `keyboard` is set, for their keyboard equivalents.
 * Returns whether a gamepad is connected.
 */
export function usePad(onAction: (a: PadAction) => void, keyboard = true): boolean {
  const handler = useRef(onAction);
  useEffect(() => {
    handler.current = onAction;
  });
  const [connected, setConnected] = useState(() => pads().length > 0);

  useEffect(() => {
    const update = () => setConnected(pads().length > 0);
    window.addEventListener('gamepadconnected', update);
    window.addEventListener('gamepaddisconnected', update);
    return () => {
      window.removeEventListener('gamepadconnected', update);
      window.removeEventListener('gamepaddisconnected', update);
    };
  }, []);

  useEffect(() => {
    if (!connected) return;
    const repeater = new PadRepeater();
    let frame = 0;
    const tick = (now: number) => {
      const held = new Set<PadAction>();
      for (const p of pads()) for (const a of heldActions(p)) held.add(a);
      for (const a of repeater.update(held, now)) handler.current(a);
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [connected]);

  useEffect(() => {
    if (!keyboard) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.altKey || e.metaKey) return;
      const target = e.target as HTMLElement | null;
      // Let text fields and ordinary buttons behave natively.
      if (target?.closest('input, textarea, select')) return;
      if (e.key === 'Enter' && target?.closest('button, a') && !target.closest('[data-pad]')) {
        return;
      }
      const action = keyAction(e.key);
      if (!action) return;
      e.preventDefault();
      handler.current(action);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [keyboard]);

  return connected;
}
