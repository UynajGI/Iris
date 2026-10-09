import { test } from 'node:test';
import assert from 'node:assert/strict';
import { UpdaterStore, UpdateCommandError, type NativeUpdaterBridge, type UpdateStatus } from '../src/updater.js';

const status = (state: UpdateStatus['state'], patch: Partial<UpdateStatus> = {}): UpdateStatus => ({
  state, current_version: '0.1.0', update: null, downloaded_bytes: 0, total_bytes: null,
  reason: state === 'not_configured' ? 'No signed update source is configured' : null, error: null, ...patch,
});
function fixture() {
  const calls: string[] = [];
  let listener: ((event: { payload: UpdateStatus }) => void) | undefined;
  let removed = 0;
  const bridge: NativeUpdaterBridge = {
    core: { async invoke<T>(command: string) { calls.push(command); return status('not_configured') as T; } },
    event: { async listen<T>(name: string, callback: (event: { payload: T }) => void) {
      assert.equal(name, 'updater:status');
      listener = callback as (event: { payload: UpdateStatus }) => void;
      return () => { removed++; };
    } },
  };
  return { bridge, calls, store: new UpdaterStore(bridge), emit: (value: UpdateStatus) => listener?.({ payload: value }), removed: () => removed };
}

test('updater connects once and only performs explicitly requested check, download and install commands', async () => {
  const { store, calls } = fixture();
  assert.deepEqual(calls, []);
  await Promise.all([store.connect(), store.connect()]);
  assert.deepEqual(calls, ['update_status']);
  assert.equal(store.getSnapshot().status?.state, 'not_configured');
  await store.check(); await store.download(); await store.install();
  assert.deepEqual(calls, ['update_status', 'check_for_update', 'download_update', 'install_update']);
  assert.equal(store.getSnapshot().pending, false);
  store.dispose();
});

test('newer progress events win over a delayed status response and preserve all update metadata', async () => {
  const { store, bridge, emit } = fixture();
  await store.connect();
  let release!: (value: UpdateStatus) => void;
  bridge.core.invoke = async <T>() => new Promise<UpdateStatus>(resolve => { release = resolve; }) as Promise<T>;
  const read = store.refresh(); await new Promise(resolve => setImmediate(resolve));
  const downloaded = status('downloaded', { downloaded_bytes: 321, total_bytes: 321,
    update: { version: '0.2.0', current_version: '0.1.0', notes: '# Changes', published_at_unix: 123456 } });
  emit(downloaded);
  release(status('available')); await read;
  assert.deepEqual(store.getSnapshot().status, downloaded);
  assert.equal(store.getSnapshot().pending, false);
  store.dispose();
});

test('structured native failures propagate without poisoning later queued commands', async () => {
  const { store, bridge } = fixture(); await store.connect();
  bridge.core.invoke = async <T>(command: string) => {
    if (command === 'download_update') throw { code: 'download_failed', message: 'Signature verification failed' };
    return status('available') as T;
  };
  await assert.rejects(store.download(), error => error instanceof UpdateCommandError && error.code === 'download_failed');
  assert.deepEqual(store.getSnapshot().error, { code: 'download_failed', message: 'Signature verification failed' });
  await store.refresh();
  assert.equal(store.getSnapshot().error, null);
  assert.equal(store.getSnapshot().status?.state, 'available');
  store.dispose();
});

test('dispose rejects late state changes and cancels queued installation before native IPC', async () => {
  const { store, bridge, emit, removed } = fixture(); await store.connect();
  const calls: string[] = [];
  let release!: (value: UpdateStatus) => void;
  bridge.core.invoke = async <T>(command: string) => {
    calls.push(command);
    return new Promise<UpdateStatus>(resolve => { release = resolve; }) as Promise<T>;
  };
  const download = store.download(); await new Promise(resolve => setImmediate(resolve));
  const install = store.install();
  store.dispose();
  emit(status('downloaded'));
  release(status('downloaded'));
  await Promise.all([download, install]);
  assert.deepEqual(calls, ['download_update']);
  assert.equal(store.getSnapshot().status?.state, 'not_configured');
  assert.equal(removed(), 1);
  assert.equal(store.getSnapshot().pending, false);
});

test('a listener that attaches after disposal is removed before any status read', async () => {
  const { store, bridge, calls } = fixture();
  let attached!: () => void;
  const hold = new Promise<void>(resolve => { attached = resolve; });
  let removed = false;
  bridge.event.listen = async () => { await hold; return () => { removed = true; }; };
  const connect = store.connect(); store.dispose(); attached(); await connect;
  assert.equal(removed, true);
  assert.deepEqual(calls, []);
});
