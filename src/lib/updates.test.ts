const relaunch = vi.fn(() => Promise.resolve());
const downloadAndInstall = vi.fn(
  async (cb: (e: { event: string; data?: Record<string, number> }) => void) => {
    cb({ event: 'Started', data: { contentLength: 100 } });
    cb({ event: 'Progress', data: { chunkLength: 40 } });
    cb({ event: 'Progress', data: { chunkLength: 60 } });
    cb({ event: 'Finished' });
  },
);
let found: unknown = null;
const check = vi.fn(() => Promise.resolve(found));

vi.mock('@tauri-apps/plugin-updater', () => ({ check }));
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch }));
vi.mock('../api/client', () => ({ isTauri: () => true }));

const { useUpdates } = await import('./updates');

describe('updates', () => {
  beforeEach(() => {
    useUpdates.setState({ status: 'idle', version: null, notes: null, progress: 0 });
    vi.clearAllMocks();
  });

  it('reports when Ignite is up to date', async () => {
    found = null;
    await useUpdates.getState().check();
    expect(useUpdates.getState().status).toBe('upToDate');
  });

  it('offers a new version, downloads it and restarts', async () => {
    found = { version: '1.5.0', body: 'Notes', downloadAndInstall };
    await useUpdates.getState().check();
    expect(useUpdates.getState()).toMatchObject({ status: 'available', version: '1.5.0' });

    await useUpdates.getState().install();
    expect(downloadAndInstall).toHaveBeenCalled();
    expect(useUpdates.getState().progress).toBe(1);
    expect(relaunch).toHaveBeenCalled();
  });

  it('stays quiet at startup when the check fails', async () => {
    check.mockImplementationOnce(() => Promise.reject(new Error('offline')));
    await useUpdates.getState().check({ silent: true });
    expect(useUpdates.getState().status).toBe('idle');
    check.mockImplementationOnce(() => Promise.reject(new Error('offline')));
    await useUpdates.getState().check();
    expect(useUpdates.getState().status).toBe('error');
  });
});
