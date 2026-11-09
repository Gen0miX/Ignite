import type { Popup } from '../api/bindings';

/** How long a popup stays on screen, in ms (before its exit animation). */
export function popupDuration(popup: Pick<Popup, 'kind'>): number {
  return popup.kind === 'achievements' ? 7000 : 6000;
}

/** Exit animation length, in ms (matches PopupCard.module.css). */
export const EXIT_MS = 320;

export interface QueueApi {
  /** Next popup, or null when the queue is empty. */
  next: () => Promise<Popup | null>;
  /** Nothing left: true when popups arrived meanwhile, false once the window closes. */
  idle: () => Promise<boolean>;
  /** Shows a popup and resolves once it has left the screen. */
  present: (popup: Popup) => Promise<void>;
}

/** Shows the backend's popups one after the other until the queue is empty. */
export async function runQueue(api: QueueApi): Promise<void> {
  for (;;) {
    const popup = await api.next();
    if (popup) {
      await api.present(popup);
      continue;
    }
    if (!(await api.idle())) return;
  }
}
