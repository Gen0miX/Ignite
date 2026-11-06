import { useEffect } from 'react';
import { useRecapQueue } from './recapQueue';
import { SessionRecapModal } from './SessionRecapModal';

/**
 * Shows the recaps in the queue one after the other. Recaps announced by a
 * notification (couch mode, another game running) open once the user is back
 * in the main window.
 */
export function SessionRecapHost() {
  const current = useRecapQueue((s) => s.queue[0]);
  const done = useRecapQueue((s) => s.done);
  const promote = useRecapQueue((s) => s.promote);

  useEffect(() => {
    promote();
    window.addEventListener('focus', promote);
    return () => window.removeEventListener('focus', promote);
  }, [promote]);

  if (!current) return null;
  return <SessionRecapModal key={current} sessionId={current} onDone={() => done(current)} />;
}
