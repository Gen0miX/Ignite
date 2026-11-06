import { useEffect, type ReactNode } from 'react';
import { listen } from '@tauri-apps/api/event';
import { isTauri } from '../api/client';
import { Outlet, useNavigate } from 'react-router-dom';
import { DragGhost } from '../lib/gameDrag';
import { usePad } from '../lib/usePad';
import { useLibraryEvents } from '../api/hooks';
import { Sidebar } from '../components/Sidebar/Sidebar';
import { Toaster } from '../components/Toaster/Toaster';
import styles from './AppShell.module.css';

/** Fluid 3-column layout: sidebar, content, and the page's optional side panel. */
export function AppShell() {
  useLibraryEvents();
  const navigate = useNavigate();
  // The Menu (Start) button of a gamepad opens couch mode from anywhere.
  usePad((a) => {
    if (a === 'menu') navigate('/couch');
  }, false);
  // "Mode Canapé" from the notification-area menu.
  useEffect(() => {
    if (!isTauri()) return;
    const off = listen('open-couch', () => navigate('/couch'));
    return () => void off.then((fn) => fn());
  }, [navigate]);
  return (
    <div className={styles.shell}>
      <Sidebar />
      <Outlet />
      <DragGhost />
      <Toaster />
    </div>
  );
}

/** Scrollable main content column; pages may render a panel next to it. */
export function PageMain({ children }: { children: ReactNode }) {
  return (
    <main className={styles.content} id="content">
      {children}
    </main>
  );
}
