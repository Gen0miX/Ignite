// Entry of the popup window (notification.html): pulls the queued popups from
// the backend and shows them one by one. See src-tauri/src/popup.rs.
import { StrictMode, useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import type { Popup } from '../api/bindings';
import { commands } from '../api/client';
import { language } from '../i18n';
import '../styles/fonts';
import '../styles/global.css';
import './popup.css';
import { PopupCard } from './PopupCard';
import { EXIT_MS, popupDuration, runQueue } from './queue';

document.documentElement.lang = language;

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

type Shown = { popup: Popup; key: number; leaving: boolean };

let started = false;

function PopupWindow() {
  const [shown, setShown] = useState<Shown | null>(null);

  useEffect(() => {
    // StrictMode mounts twice in dev: run the queue once.
    if (started) return;
    started = true;
    let key = 0;
    let revealed = false;
    void runQueue({
      next: () => commands.popupNext(),
      idle: () => commands.popupIdle(),
      present: async (popup) => {
        key += 1;
        setShown({ popup, key, leaving: false });
        if (!revealed) {
          revealed = true;
          // Let the first frame paint before showing the window.
          await new Promise((resolve) => requestAnimationFrame(() => setTimeout(resolve, 16)));
          await commands.popupReady();
        }
        await sleep(popupDuration(popup));
        setShown((s) => (s ? { ...s, leaving: true } : s));
        await sleep(EXIT_MS);
        setShown(null);
      },
    });
  }, []);

  if (!shown) return null;
  return (
    <PopupCard
      key={shown.key}
      popup={shown.popup}
      duration={popupDuration(shown.popup)}
      leaving={shown.leaving}
    />
  );
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <PopupWindow />
  </StrictMode>,
);
