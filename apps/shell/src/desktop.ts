import { IrisClient } from './client.js';
import { IrisStore } from './store.js';
import { Transport, type Session } from './transport.js';
import { CommandSystem } from './commands.js';
import { createAppTitleSynchronizer } from './native-title.js';
import { openProjectFolder } from './native-dialog.js';
import { ViewPreferencesRepository } from './view-preferences.js';
import { UpdaterStore } from './updater.js';

const viewPreferences = new ViewPreferencesRepository();
let connectionGeneration = 0;
let reconnectProjectId: number | undefined;

interface TauriBridge {
  core: { invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> };
  event: { listen<T>(event: string, callback: (event: { payload: T }) => void): Promise<() => void> };
}
declare global {
  interface Window {
    __TAURI__?: TauriBridge;
    iris?: { store: IrisStore; commands: CommandSystem; updater: UpdaterStore; openFolder(): Promise<boolean> };
  }
}
/** Future presentation mounts into #root; this entrypoint has no screen or style. */
export async function connectDesktop(bridge: TauriBridge): Promise<void> {
  const generation = ++connectionGeneration;
  const previousId = window.iris?.store.getSnapshot().project?.id ?? reconnectProjectId;
  reconnectProjectId = previousId;
  window.iris?.store.dispose();
  const session = await bridge.core.invoke<Session>('daemon_session');
  if (generation !== connectionGeneration) return;
  const store = new IrisStore(new IrisClient(new Transport(session)), viewPreferences);
  const updater = window.iris?.updater ?? new UpdaterStore(bridge);
  window.iris = { store, commands: new CommandSystem(store), updater, openFolder: () => openProjectFolder(bridge.core, store) };
  // Reading native updater state never initiates a network check or download.
  void updater.connect().catch(() => {}); // The updater exposes its own structured error state.
  try {
    await store.initialize();
    if (generation !== connectionGeneration) { store.dispose(); return; }
    if (previousId !== undefined) await store.restoreProject(previousId);
    if (generation !== connectionGeneration) { store.dispose(); return; }
  } catch (error) {
    store.dispose();
    if (generation === connectionGeneration) throw error;
    return;
  }
  store.connect();
  window.dispatchEvent(new CustomEvent('iris:ready', { detail: window.iris }));
}
if (typeof window !== 'undefined' && window.__TAURI__) {
  const bridge = window.__TAURI__;
  const report = (error: unknown) => window.dispatchEvent(new CustomEvent('iris:error', { detail: String(error) }));
  const synchronizeTitle = createAppTitleSynchronizer(bridge.core, document);
  const updateTitle = () => { void synchronizeTitle(navigator.language).catch(report); };
  updateTitle();
  window.addEventListener('languagechange', updateTitle);
  void connectDesktop(bridge).catch(report);
  void bridge.event.listen('daemon:restarted', () => { void connectDesktop(bridge).catch(report); });
  void bridge.event.listen('daemon:unavailable', () => window.iris?.store.disconnect());
  window.addEventListener('beforeunload', () => { window.iris?.store.dispose(); window.iris?.updater.dispose(); });
}
