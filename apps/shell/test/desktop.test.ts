import { test } from 'node:test';
import assert from 'node:assert/strict';
import { IrisStore, ViewPreferencesRepository } from '../src/index.js';

test('desktop reconnect restores preferences without recording opens and ignores overlapping old sessions', async () => {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const originalFetch = globalThis.fetch;
  const originalConnect = IrisStore.prototype.connect;
  const values = new Map<string, string>();
  const storage = { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => { values.set(key, value); } };
  new ViewPreferencesRepository(storage).save('C:/project1', { filter: { sort: 'date', descending: true, limit: 50 }, reviewOpen: true });
  let ready = 0; let connections = 0;
  const testWindow = { localStorage: storage, iris: { store: { getSnapshot: () => ({ project: { id: 1 } }), dispose() {} } }, dispatchEvent: () => { ready++; return true; } } as unknown as Window & typeof globalThis;
  Object.defineProperty(globalThis, 'window', { configurable: true, writable: true, value: testWindow });
  const methods: string[] = [];
  let holdBootstrap: Promise<Response> | undefined;
  let bootstrapStarted: (() => void) | undefined;
  const project = { id: 1, root: 'C:/project1', name: 'project1', cache_root: 'cache', created_at: '', groups_dirty: false, pending_analysis: 0, hidden: false, last_opened_at: 'original' };
  globalThis.fetch = async (input, init) => {
    methods.push(init?.method ?? 'GET');
    const path = new URL(String(input)).pathname;
    if (path.endsWith('/bootstrap') && holdBootstrap) {
      const pending = holdBootstrap; holdBootstrap = undefined; bootstrapStarted?.(); return pending;
    }
    const value = path.endsWith('/bootstrap') ? { version: '0.1.0', capabilities: ['jpeg'] }
      : path.endsWith('/projects') ? [project]
      : path.endsWith('/projects/1') ? project
      : path.endsWith('/models') ? { selected: 'yunet', detectors: [] }
      : path.endsWith('/settings') ? {}
      : path.endsWith('/progress') ? { kind: 'analysis', state: 'completed', errors: [] } : [];
    return Response.json(value);
  };
  IrisStore.prototype.connect = function () { connections++; };
  let updaterReads = 0;
  const bridge = { core: { invoke: async <T>(command: string) => {
    if (command === 'update_status') {
      updaterReads++;
      return { state: 'not_configured', current_version: '0.1.0', update: null, downloaded_bytes: 0, total_bytes: null, reason: 'No update source', error: null } as T;
    }
    assert.equal(command, 'daemon_session');
    return { base_url: 'http://127.0.0.1:45678', token: 'secret', version: '0.1.0' } as T;
  } }, event: { listen: async () => () => {} } };
  try {
    const { connectDesktop } = await import('../src/desktop.js');
    await connectDesktop(bridge);
    const updater = testWindow.iris?.updater;
    assert.equal(testWindow.iris?.store.getSnapshot().filter.sort, 'date');
    assert.equal(testWindow.iris?.store.getSnapshot().reviewOpen, true);
    assert.equal(testWindow.iris?.store.getSnapshot().project?.last_opened_at, 'original');
    let release!: (response: Response) => void;
    holdBootstrap = new Promise(resolve => { release = resolve; });
    const started = new Promise<void>(resolve => { bootstrapStarted = resolve; });
    const old = connectDesktop(bridge); await started;
    await connectDesktop(bridge);
    const current = testWindow.iris;
    release(Response.json({ version: '0.1.0', capabilities: ['jpeg'] })); await old;
    assert.equal(testWindow.iris, current);
    assert.equal(current?.store.getSnapshot().project?.id, 1);
    assert.equal(current?.store.getSnapshot().filter.sort, 'date');
    assert.equal(ready, 2);
    assert.equal(connections, 2);
    assert.equal(current?.updater, updater, 'native updater survives daemon reconnects');
    assert.equal(updaterReads, 1, 'reconnect does not create another updater subscription or check');
    assert.ok(methods.every(method => method === 'GET'));
  } finally {
    testWindow.iris?.store.dispose();
    testWindow.iris?.updater.dispose();
    globalThis.fetch = originalFetch;
    IrisStore.prototype.connect = originalConnect;
    if (descriptor) Object.defineProperty(globalThis, 'window', descriptor);
    else Reflect.deleteProperty(globalThis, 'window');
  }
});
