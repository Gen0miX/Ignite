import { Outlet } from 'react-router-dom';
import { Sidebar } from '../components/Sidebar/Sidebar';
import styles from './AppShell.module.css';

/** Fluid 3-column layout: sidebar, content, (later) selected-game panel. */
export function AppShell() {
  return (
    <div className={styles.shell}>
      <Sidebar />
      <main className={styles.content} id="content">
        <Outlet />
      </main>
    </div>
  );
}
