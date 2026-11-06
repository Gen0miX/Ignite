import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useQueryClient } from '@tanstack/react-query';
import { commands, isTauri, unwrap } from '../../api/client';
import { fr } from '../../i18n/fr';
import { formatPlaytimeLong } from '../../lib/format';
import { usePrefs } from '../../lib/prefs';
import { recapDecision, useRecapQueue } from './recapQueue';

interface RecapEvent {
  sessionId: string;
  gameId: string;
}

export const recapKey = (sessionId: string) => ['session-recap', sessionId] as const;

/**
 * Listens to the tracker's recap events (mounted once, above the router, so
 * it also runs in couch mode) and opens, announces or skips each recap.
 */
export function useRecapEvents(isCouch: () => boolean) {
  const client = useQueryClient();
  useEffect(() => {
    if (!isTauri()) return;
    const offs: Promise<() => void>[] = [];

    offs.push(
      listen<RecapEvent>('session-recap-ready', async ({ payload }) => {
        const recap = await client
          .fetchQuery({
            queryKey: recapKey(payload.sessionId),
            queryFn: () => unwrap(commands.sessionRecap(payload.sessionId)),
          })
          .catch(() => null);
        if (!recap) return;
        const active = await commands.trackerActive().catch(() => []);
        const { recapMode, recapMinMinutes } = usePrefs.getState();
        const decision = recapDecision({
          mode: recapMode,
          minMinutes: recapMinMinutes,
          activeSeconds: recap.activeSeconds,
          couch: isCouch(),
          otherGameRunning: active.length > 0,
        });
        const queue = useRecapQueue.getState();
        if (decision === 'open') {
          await commands.windowReveal().catch(() => undefined);
          queue.open(payload.sessionId);
        } else if (decision === 'notify') {
          queue.defer(payload.sessionId);
          void commands.notify(
            fr.recap.notifyTitle,
            fr.recap.notifyBody(recap.game.title, formatPlaytimeLong(recap.activeSeconds / 60)),
          );
        }
      }),
    );
    // Achievements resynced after the session: refresh the recap.
    offs.push(
      listen<RecapEvent>('session-recap-updated', ({ payload }) => {
        void client.invalidateQueries({ queryKey: recapKey(payload.sessionId) });
      }),
    );
    return () => offs.forEach((off) => void off.then((fn) => fn()));
  }, [client, isCouch]);
}
