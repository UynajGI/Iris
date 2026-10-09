import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Transport, ApiError, IrisClient, IrisStore, CommandSystem, EventBus, productName } from '../src/index.js';
import type { Socket, Photo, Action, Settings, ModelStatusResponse, Face, VisionAnalysis, Progress } from '../src/index.js';

const session = { base_url: 'http://127.0.0.1:45678', token: 'local-secret', version: '0.1.0' };
const settings: Settings = { execution_provider: 'cpu', directml_device_id: null, embedding_provider: 'none', embedding_model_sha256: null, semantic_similarity_threshold: null, occlusion_provider: 'none', occlusion_model_sha256: null, occlusion_min_visible_fraction: null, face_detector: 'yunet', scrfd_model_sha256: null, enable_niqe: true, niqe_weight: 0.2, sharpness_weight: 0.35, eyes_weight: 0.3, face_weight: 0.15, exposure_weight: 0.2, smile_weight: 0, face_confidence: 0.7, max_faces: 20, recommend_threshold: 0.7, reject_threshold: 0.3 };
const modelStatus = (selected: ModelStatusResponse['selected'] = 'yunet', occlusion_selected: ModelStatusResponse['occlusion_selected'] = 'none'): ModelStatusResponse => ({ selected, occlusion_selected, embedding_selected: 'none', embeddings: [], occlusion: [
  { provider: 'none', state: 'disabled', sha256: null, reason: 'FaceOcc is not enabled or checked' },
  { provider: 'faceocc', state: 'missing', sha256: null, reason: 'User-supplied optional FaceOcc is missing' },
], detectors: [
  { provider: 'yunet', state: 'available', sha256: 'a'.repeat(64), reason: null },
  { provider: 'scrfd_500m', state: 'missing', sha256: null, reason: 'User-supplied optional model is missing' },
] });
const photo = (id: number, project_id = 1): Photo => ({ id, project_id, filename: `${id}.jpg`, path: `${id}.jpg`, format: 'jpeg', mtime: 1, size_bytes: 100, width: 10, height: 10, taken_at: null, capture_variant_id: null, missing: false, quarantined: false, decision: 'pending', analysis: null, analysis_status: 'missing' });
function fixture(count = 3) {
  let photos = Array.from({ length: count }, (_, index) => photo(index + 1));
  let groupsDirty = false;
  let projectSettings = { ...settings };
  const history: Photo[][] = [];
  const requests: { path: string; method: string; body: Record<string, unknown> }[] = [];
  let profiles: { name: string; settings: Record<string, unknown> }[] = [];
  const fetcher: typeof fetch = async (input, init) => {
    assert.equal(new Headers(init?.headers).get('authorization'), 'Bearer local-secret');
    const url = new URL(String(input));
    const path = url.pathname.replace('/api/v1', '');
    const body = JSON.parse(String(init?.body ?? '{}')) as Record<string, unknown>;
    requests.push({ path, method: init?.method ?? 'GET', body });
    let result: unknown = {};
    if (path === '/bootstrap') result = { version: '0.1.0', capabilities: ['jpeg'] };
    else if (path === '/projects' && init?.method === 'GET') result = [];
    else if (path === '/projects') result = { id: 1, root: body.root, name: 'test', created_at: '', cache_root: 'cache', groups_dirty: groupsDirty, pending_analysis: 3, hidden: false, last_opened_at: '' };
    else if (path === '/projects/1') result = { id: 1, root: 'C:/photos', name: 'test', created_at: '', cache_root: 'cache', groups_dirty: groupsDirty, pending_analysis: 3, hidden: false, last_opened_at: '' };
    else if (path === '/settings') { assert.equal(url.searchParams.get('project_id'), '1'); if (init?.method === 'PUT') { groupsDirty = true; projectSettings = body as Settings; } result = projectSettings; }
    else if (path === '/models') { assert.ok(url.searchParams.has('project_id')); result = modelStatus(projectSettings.face_detector, projectSettings.occlusion_provider); }
    else if (path === '/profiles' && init?.method === 'POST') { profiles.push(body as typeof profiles[number]); result = body; }
    else if (path === '/profiles') result = profiles;
    else if (path.endsWith('/apply')) { assert.equal(body.project_id, 1); groupsDirty = true; projectSettings = { ...settings, sharpness_weight: 0.7 }; result = projectSettings; }
    else if (path.startsWith('/profiles/') && init?.method === 'DELETE') { profiles = []; }
    else if (path.endsWith('/photos')) {
      let matching = photos.filter(p => !url.searchParams.has('decision') || p.decision === url.searchParams.get('decision'));
      if (url.searchParams.get('descending') === 'true') matching.reverse();
      const offset = Number(url.searchParams.get('offset') ?? 0);
      result = matching.slice(offset, offset + Number(url.searchParams.get('limit') ?? 200));
    }
    else if (path.endsWith('/groups')) result = groupsDirty ? [] : [{ id: 'g', project_id: 1, kind: 'burst', member_photo_ids: [1, 2] }];
    else if (path.endsWith('/quarantine')) result = [{ id: 'previous-manifest', project_id: 1, state: 'interrupted', items: [] }];
    else if (path.endsWith('/progress') || /\/(scan|analyze|cancel|pause|resume)$/.test(path)) result = { kind: 'scan', state: path.endsWith('/cancel') ? 'cancelled' : 'idle', completed: 0, total: 3, errors: [], result: null };
    else if (path.endsWith('/decisions')) {
      history.push(structuredClone(photos));
      photos = photos.map(p => (body.photo_ids as number[]).includes(p.id) ? { ...p, decision: body.action as Action } : p);
      result = { id: 'batch', changed: (body.photo_ids as number[]).length, conflicts: [] };
    } else if (path.endsWith('/undo')) { photos = history.pop() ?? photos; result = { id: 'batch', changed: 1, conflicts: [] }; }
    else if (path.endsWith('/quarantine/preview')) result = { id: 'manifest', project_id: 1, state: 'preview', items: [] };
    else if (path.endsWith('/quarantine/commit')) result = { id: 'manifest', project_id: 1, state: 'committed', items: [] };
    else if (path.includes('/export/')) result = { written: 1, skipped: 0, paths: ['test'] };
    return Response.json(result);
  };
  const client = new IrisClient(new Transport(session, fetcher));
  return { client, store: new IrisStore(client), requests };
}

test('library selection, serialized decisions, undo and groups round-trip through API', async () => {
  const { store, requests } = fixture();
  await store.initialize(); await store.openProject('C:/photos');
  assert.equal(store.getSnapshot().photos.length, 3);
  assert.equal(store.getSnapshot().groups[0]?.member_photo_ids.length, 2);
  assert.equal(store.getSnapshot().quarantinePlans[0]?.state, 'interrupted');
  const commands = new CommandSystem(store);
  await Promise.all([commands.dispatch('keep'), commands.dispatch('reject')]);
  assert.deepEqual(store.getSnapshot().photos.map(p => p.decision), ['keep', 'reject', 'pending']);
  assert.equal(store.getSnapshot().focusedId, 3);
  await commands.dispatch('undo');
  assert.equal(store.getSnapshot().photos[1]?.decision, 'pending');
  store.select([1, 3, 3, 999]); await commands.dispatch('flag');
  assert.deepEqual(store.getSnapshot().photos.map(p => p.decision), ['flag', 'pending', 'flag']);
  await store.setFilter({ decision: 'flag' });
  assert.equal(store.getSnapshot().photos.length, 2);
  assert.equal(requests.filter(r => r.path.endsWith('/decisions')).length, 3);
  assert.equal(store.getSnapshot().pending, 0);
});

test('profiles, cancellation, export and explicit quarantine confirmation', async () => {
  const { store, client, requests } = fixture(); await store.openProject('C:/photos');
  await store.saveProfile({ name: 'portrait / 中文', settings: { ...settings, sharpness_weight: 0.7 } });
  assert.equal(store.getSnapshot().profiles.length, 1);
  await store.applyProfile('portrait / 中文');
  assert.equal(store.getSnapshot().settings?.sharpness_weight, 0.7);
  await store.deleteProfile('portrait / 中文');
  assert.equal(store.getSnapshot().profiles.length, 0);
  await store.run('scan'); await store.run('cancel');
  assert.ok(requests.some(r => r.path.endsWith('/cancel')));
  assert.equal((await client.exportCopy(1, 'C:/selected')).written, 1);
  await assert.rejects(store.commitQuarantine('guessed'), /Preview and confirm/);
  assert.equal(requests.filter(r => r.path.endsWith('/quarantine/commit')).length, 0);
  await store.previewQuarantine(); await store.commitQuarantine('manifest');
  assert.equal(store.getSnapshot().quarantine?.state, 'committed');
});

test('transport authenticates media and errors without leaking token into URLs', async () => {
  let address = '';
  const transport = new Transport(session, async (input, init) => {
    address = String(input); assert.equal(new Headers(init?.headers).get('authorization'), 'Bearer local-secret');
    return new Response('forbidden', { status: 403 });
  });
  await assert.rejects(transport.blob('/api/v1/photos/1/thumb'), error => error instanceof ApiError && error.status === 403);
  assert.ok(!address.includes(session.token));
  assert.throws(() => new Transport({ ...session, base_url: 'https://example.com' }), /loopback/);
});

test('keyboard ignores text editing and repeated events; configurable bindings work', async () => {
  const { store } = fixture(); await store.openProject('C:/photos');
  const commands = new CommandSystem(store, { q: 'next' });
  let prevented = false;
  const event = { key: 'q', target: { tagName: 'INPUT' }, preventDefault: () => { prevented = true; } } as unknown as KeyboardEvent;
  assert.equal(commands.handle(event), false);
  assert.equal(prevented, false);
  assert.equal(commands.handle({ ...event, target: null } as unknown as KeyboardEvent), true);
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(store.getSnapshot().focusedId, 2);
  assert.equal(productName('zh-TW'), '伊人'); assert.equal(productName('en-GB'), 'IrisVision');
});

class FakeSocket implements Socket {
  onopen: Socket['onopen'] = null; onmessage: Socket['onmessage'] = null;
  onerror: Socket['onerror'] = null; onclose: Socket['onclose'] = null;
  close() { this.onclose?.({} as CloseEvent); }
}
test('event bus authenticates subprotocol, reconnects, resynchronizes and cleans timers', async () => {
  const sockets: FakeSocket[] = []; const states: string[] = []; const events: unknown[] = [];
  let syncs = 0;
  const bus = new EventBus(session, { retryMs: 1, pollMs: 5,
    createSocket: (url, protocols) => { assert.ok(!url.includes(session.token)); assert.deepEqual(protocols, ['iris', 'iris-token.local-secret']); const socket = new FakeSocket(); sockets.push(socket); return socket; },
    synchronize: async () => { syncs++; }, onState: state => states.push(state), onEvent: event => events.push(event),
  });
  bus.start(); sockets[0]?.onopen?.({} as Event);
  sockets[0]?.onmessage?.({ data: JSON.stringify({ event: 'scan:progress', project_id: 1, data: {} }) } as MessageEvent);
  sockets[0]?.close();
  await new Promise(resolve => setTimeout(resolve, 20));
  assert.equal(sockets.length, 2); assert.ok(syncs >= 1); assert.equal(events.length, 1);
  sockets[1]?.onopen?.({} as Event);
  assert.ok(states.includes('reconnecting'));
  bus.stop(); const count = syncs;
  await new Promise(resolve => setTimeout(resolve, 15));
  assert.equal(syncs, count); assert.equal(states.at(-1), 'disconnected');
});

test('failed writes leave decisions intact and clear busy state', async () => {
  const { store, client } = fixture(); await store.openProject('C:/photos');
  client.decide = async () => { throw new ApiError(500, { error: 'disk full' }); };
  await assert.rejects(store.decide('reject'), /500/);
  assert.equal(store.getSnapshot().photos[0]?.decision, 'pending');
  assert.equal(store.getSnapshot().pending, 0);
  assert.match(store.getSnapshot().error ?? '', /500/);
});

test('default transport preserves the native Window fetch receiver for JSON and media', async () => {
  const original = globalThis.fetch;
  let calls = 0;
  globalThis.fetch = async function (this: typeof globalThis) {
    assert.equal(this, globalThis, 'WebView2 fetch requires its native global receiver');
    calls++;
    return Response.json({ ok: true });
  };
  try {
    const transport = new Transport(session);
    assert.deepEqual(await transport.request('GET', '/api/v1/bootstrap'), { ok: true });
    assert.ok((await transport.blob('/api/v1/photos/1/thumb')).size > 0);
    assert.equal(calls, 2);
  } finally { globalThis.fetch = original; }
});

test('failed scan preserves terminal errors and refreshes successfully imported photos', async () => {
  const { store, client } = fixture(0); await store.openProject('C:/photos');
  const errors = ['broken.jpg: invalid JPEG'];
  const report = { added: 1, changed: 0, unchanged: 0, missing: 0, skipped: 0, cancelled: false, errors };
  const terminal: Progress = { kind: 'scan', state: 'failed', completed: 1, total: 1, errors, result: report };
  let photoReads = 0;
  client.photos = async () => { photoReads++; return [photo(7)]; };
  client.progress = async () => structuredClone(terminal);
  const socket: Socket = { onopen: null, onmessage: null, onerror: null, onclose: null, close() {} };
  // No open event or timer poll: only the failed scan event can trigger this refresh.
  store.connect({ createSocket: () => socket, pollMs: 60000 });
  let unsubscribe = () => {};
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    const refreshed = new Promise<void>((resolve, reject) => {
      timer = setTimeout(() => reject(new Error('failed scan did not refresh partial library')), 2000);
      unsubscribe = store.subscribe(() => {
        if (store.getSnapshot().photos.some(photo => photo.id === 7)) resolve();
      });
    });
    socket.onmessage?.({ data: JSON.stringify({ event: 'scan:progress', project_id: 1, data: terminal }) } as MessageEvent);
    assert.equal(store.getSnapshot().progress?.state, 'failed');
    assert.deepEqual(store.getSnapshot().progress?.errors, errors, 'terminal errors are visible before the refresh');
    await refreshed;
    assert.ok(photoReads > 0);
    assert.deepEqual(store.getSnapshot().photos.map(photo => photo.id), [7]);
    assert.equal(store.getSnapshot().focusedId, 7);
    assert.deepEqual(store.getSnapshot().progress, terminal, 'refresh retains failure, counters, errors and the partial scan report');
  } finally { clearTimeout(timer); unsubscribe(); store.dispose(); }
});

test('a slower previous filter response cannot replace a newer library snapshot', async () => {
  const { store, client } = fixture(); await store.openProject('C:/photos');
  let release!: (photos: Photo[]) => void;
  client.photos = async (_id, filter) => filter?.decision === 'keep' ? new Promise(resolve => { release = resolve; }) : [photo(3)];
  const oldRequest = store.setFilter({ decision: 'keep' });
  await store.setFilter({ decision: 'reject' });
  release([photo(1)]); await oldRequest;
  assert.deepEqual(store.getSnapshot().photos.map(p => p.id), [3]);
  assert.equal(store.getSnapshot().filter.decision, 'reject');
});

test('settings/profile invalidate groups and persisted dirty state survives reopening', async () => {
  const { store, client, requests } = fixture();
  await store.openProject('C:/photos');
  assert.equal(store.getSnapshot().groups.length, 1);
  await store.saveSettings({ ...settings, sharpness_weight: 0.5 });
  assert.equal(store.getSnapshot().groups.length, 0);
  assert.equal(store.getSnapshot().reanalysisRequired, true);

  // Empty groups alone do not prove staleness. This empty project still needs
  // rebuilding because the server persisted groups_dirty, even after reopen.
  client.photos = async () => [];
  const reopened = new IrisStore(client);
  await reopened.openProject('C:/photos');
  assert.equal(reopened.getSnapshot().project?.groups_dirty, true);
  assert.equal(reopened.getSnapshot().reanalysisRequired, true);
  await reopened.applyProfile('portrait');
  assert.equal(reopened.getSnapshot().groups.length, 0);
  assert.equal(reopened.getSnapshot().reanalysisRequired, true);
  await reopened.reanalyze();
  assert.ok(requests.some(request => request.path.endsWith('/analyze')));
  // Merely starting an analysis does not claim the stale state is resolved.
  assert.equal(reopened.getSnapshot().reanalysisRequired, true);

  const project = reopened.getSnapshot().project!;
  client.project = async () => ({ ...project, groups_dirty: false, pending_analysis: 0 });
  await reopened.refresh();
  assert.equal(reopened.getSnapshot().groups.length, 0);
  assert.equal(reopened.getSnapshot().reanalysisRequired, false);
});

test('late settings/profile responses cannot overwrite a newly opened project', async () => {
  for (const operation of ['settings', 'profile'] as const) {
    const { store, client } = fixture();
    await store.openProject('C:/photos');
    let release!: (value: Settings) => void;
    const delayed = () => new Promise<Settings>(resolve => { release = resolve; });
    if (operation === 'settings') client.saveSettings = delayed;
    else client.applyProfile = delayed;
    const previous = operation === 'settings' ? store.saveSettings({ ...settings, sharpness_weight: 0.4 }) : store.applyProfile('old-project');
    await new Promise(resolve => setImmediate(resolve)); // Let the queued write reach the server.
    const other = { id: 2, root: 'C:/other', name: 'other', created_at: '', cache_root: 'cache2', groups_dirty: false, pending_analysis: 0, hidden: false, last_opened_at: '' };
    const otherSettings = { ...settings, sharpness_weight: 0.9 };
    client.openProject = async () => other;
    client.project = async () => other;
    client.settings = async () => otherSettings;
    client.photos = async () => [];
    client.groups = async () => [];
    client.quarantinePlans = async () => [];
    await store.openProject(other.root);
    release({ ...settings, sharpness_weight: 0.4 });
    await previous;
    assert.equal(store.getSnapshot().project?.id, 2);
    assert.deepEqual(store.getSnapshot().settings, otherSettings);
    assert.equal(store.getSnapshot().reanalysisRequired, false);
    assert.equal(store.getSnapshot().pending, 0);
  }
});

test('late failure from another project does not replace the active project error state', async () => {
  const { store, client } = fixture(); await store.openProject('C:/photos');
  let reject!: (error: Error) => void;
  client.saveSettings = async () => new Promise((_resolve, fail) => { reject = fail; });
  const write = store.saveSettings(settings);
  await new Promise(resolve => setImmediate(resolve));
  // Reopening the same ID creates a new view generation too.
  await store.openProject('C:/photos');
  reject(new Error('old view failed'));
  await assert.rejects(write, /old view failed/);
  assert.equal(store.getSnapshot().error, null);
});

test('overlapping settings and profile commands commit in order and retain the latest canonical settings', async () => {
  for (const firstKind of ['settings', 'profile'] as const) {
    for (const secondKind of ['settings', 'profile'] as const) {
      const { store, client } = fixture(); await store.openProject('C:/photos');
      const save = client.saveSettings;
      const writes: number[] = [];
      let release!: () => void;
      const hold = new Promise<void>(resolve => { release = resolve; });
      let started!: () => void;
      const committed = new Promise<void>(resolve => { started = resolve; });
      const write = async (id: number, value: Settings) => {
        writes.push(value.sharpness_weight);
        const canonical = await save(id, value);
        if (writes.length === 1) { started(); await hold; }
        return canonical;
      };
      client.saveSettings = write;
      client.applyProfile = (id, name) => write(id, { ...settings, sharpness_weight: name === 'first' ? 0.4 : 0.9 });
      const first = firstKind === 'settings' ? store.saveSettings({ ...settings, sharpness_weight: 0.4 }) : store.applyProfile('first');
      await committed;
      const request = { ...settings, sharpness_weight: 0.9 };
      const second = secondKind === 'settings' ? store.saveSettings(request) : store.applyProfile('second');
      request.sharpness_weight = 0.1; // Queued settings must preserve the submitted value.
      await new Promise(resolve => setImmediate(resolve));
      assert.deepEqual(writes, [0.4], 'a second write cannot race the first response');
      release(); await Promise.all([first, second]);
      assert.deepEqual(writes, [0.4, 0.9]);
      assert.deepEqual(store.getSnapshot().settings, await client.settings(1));
      assert.equal(store.getSnapshot().settings?.sharpness_weight, 0.9);
      assert.equal(store.getSnapshot().pending, 0);
    }
  }
});

test('queued settings from a retired project are skipped and a failed write does not block later settings', async () => {
  const { store, client } = fixture(); await store.openProject('C:/photos');
  const save = client.saveSettings;
  let reject!: (reason: Error) => void;
  let started!: () => void;
  const ready = new Promise<void>(resolve => { started = resolve; });
  client.saveSettings = async () => { started(); return new Promise((_resolve, fail) => { reject = fail; }); };
  const first = store.saveSettings(settings);
  const rejected = assert.rejects(first, /first write failed/);
  await ready;
  const retired = store.saveSettings({ ...settings, sharpness_weight: 0.4 });
  await store.openProject('C:/photos');
  const writes: number[] = [];
  client.saveSettings = (id, value) => { writes.push(value.sharpness_weight); return save(id, value); };
  const current = store.saveSettings({ ...settings, sharpness_weight: 0.9 });
  reject(new Error('first write failed'));
  await Promise.all([rejected, retired, current]);
  assert.deepEqual(writes, [0.9], 'the queued old-project write must never reach the server');
  assert.equal(store.getSnapshot().settings?.sharpness_weight, 0.9);
  assert.equal(store.getSnapshot().error, null);
  assert.equal(store.getSnapshot().pending, 0);
});

test('an accepted settings change with failed refresh preserves history but withholds current analysis', async () => {
  const { store, client } = fixture();
  const analysis: VisionAnalysis = {
    version: 'test-current', width: 10, height: 10, original_width: 10, original_height: 10,
    orientation: 1, sharpness_lap: 100, sharpness_fft: 10,
    exposure: { mean: 127, highlight_clip: 0, shadow_clip: 0, verdict: 'normal' },
    faces: [], composite_score: 88, verdict: 'recommend', phash: '0', structure: [], warnings: [],
    score_breakdown: { method: 'server', effective_weight_total: 1, components: [] },
  };
  client.photos = async () => [{ ...photo(1), analysis_status: 'current', analysis }, photo(2)];
  await store.openProject('C:/photos');
  client.saveSettings = async () => { throw new Error('write rejected'); };
  await assert.rejects(store.saveSettings(settings), /write rejected/);
  assert.equal(store.photoAnalysis(1)?.score, 88, 'a rejected write leaves existing analysis intact');
  let release!: () => void;
  let refreshStarted!: () => void;
  const started = new Promise<void>(resolve => { refreshStarted = resolve; });
  const hold = new Promise<void>(resolve => { release = resolve; });
  client.saveSettings = async (_id, value) => value;
  client.photos = async () => { refreshStarted(); await hold; throw new Error('refresh offline'); };
  const write = store.saveSettings({ ...settings, face_detector: 'scrfd_500m', scrfd_model_sha256: 'a'.repeat(64) });
  const failed = assert.rejects(write, /refresh offline/);
  await started;
  assert.equal(store.photoAnalysis(1)?.status, 'stale', 'invalidation happens before waiting for refresh');
  release(); await failed;
  const readout = store.photoAnalysis(1)!;
  assert.equal(store.getSnapshot().settings?.face_detector, 'scrfd_500m');
  assert.equal(store.getSnapshot().reanalysisRequired, true);
  assert.equal(readout.analysis, analysis);
  assert.equal(readout.current, null);
  assert.equal(readout.score, null);
  assert.equal(readout.verdict, null);
  assert.equal(readout.scoreBreakdown, null);
  assert.equal(store.photoAnalysis(2)?.status, 'missing');
  assert.match(store.getSnapshot().error ?? '', /refresh offline/);
  // A later authoritative refresh can restore current results after recovery.
  client.photos = async () => [{ ...photo(1), analysis_status: 'current', analysis }];
  await store.refresh();
  assert.equal(store.photoAnalysis(1)?.score, 88);
});

test('reanalysis status uses full-project counters rather than the visible page', async () => {
  const { store, client } = fixture(); await store.openProject('C:/photos');
  const project = store.getSnapshot().project!;
  client.photos = async () => [];
  client.groups = async () => [];
  client.project = async () => ({ ...project, groups_dirty: false, pending_analysis: 12 });
  await store.refresh();
  assert.equal(store.getSnapshot().reanalysisRequired, true);
  client.project = async () => ({ ...project, groups_dirty: true, pending_analysis: 0 });
  await store.refresh();
  assert.equal(store.getSnapshot().reanalysisRequired, true);
  client.project = async () => ({ ...project, groups_dirty: false, pending_analysis: 0 });
  await store.refresh();
  assert.equal(store.getSnapshot().reanalysisRequired, false);
});

test('detector status preserves missing/invalid SCRFD selection without fallback or repeated polling', async () => {
  const { store, client, requests } = fixture();
  await store.openProject('C:/photos');
  assert.equal(store.getSnapshot().detectorModels?.selected, 'yunet');
  await store.saveSettings({ ...settings, face_detector: 'scrfd_500m', scrfd_model_sha256: 'b'.repeat(64) });
  assert.equal(store.getSnapshot().settings?.face_detector, 'scrfd_500m');
  assert.equal(store.getSnapshot().detectorModels?.selected, 'scrfd_500m');
  assert.equal(store.getSnapshot().detectorModels?.detectors[1]?.state, 'missing');
  const reads = requests.filter(request => request.path === '/models').length;
  await store.refresh(); await store.refresh();
  assert.equal(requests.filter(request => request.path === '/models').length, reads);
  const invalid = modelStatus('scrfd_500m');
  invalid.detectors[1] = { provider: 'scrfd_500m', state: 'invalid', sha256: 'c'.repeat(64), reason: 'Declared hash mismatch' };
  client.models = async () => invalid;
  await store.refreshDetectorModels();
  assert.deepEqual(store.getSnapshot().detectorModels, invalid);
  assert.equal(store.getSnapshot().settings?.face_detector, 'scrfd_500m');
});

test('late detector status cannot replace a newer status or a reopened project', async () => {
  const { store, client } = fixture(); await store.openProject('C:/photos');
  for (const reopen of [false, true]) {
    let release!: (value: ModelStatusResponse) => void;
    client.models = async () => new Promise(resolve => { release = resolve; });
    const previous = store.refreshDetectorModels();
    client.models = async () => modelStatus('yunet');
    if (reopen) await store.openProject('C:/photos');
    else await store.refreshDetectorModels();
    release(modelStatus('scrfd_500m')); await previous;
    assert.equal(store.getSnapshot().detectorModels?.selected, 'yunet');
  }
});

test('FaceOcc defaults to disabled and missing or invalid status never silently disables the selection', async () => {
  const { store, client } = fixture(); await store.openProject('C:/photos');
  assert.equal(store.getSnapshot().settings?.occlusion_provider, 'none');
  assert.equal(store.getSnapshot().detectorModels?.occlusion_selected, 'none');
  assert.equal(store.getSnapshot().detectorModels?.occlusion[0]?.state, 'disabled');
  // Explicit test threshold, not an accuracy-calibrated product default.
  await store.saveSettings({ ...settings, occlusion_provider: 'faceocc', occlusion_model_sha256: 'b'.repeat(64), occlusion_min_visible_fraction: 0.75 });
  assert.equal(store.getSnapshot().detectorModels?.occlusion_selected, 'faceocc');
  assert.equal(store.getSnapshot().detectorModels?.occlusion[1]?.state, 'missing');
  assert.equal(store.getSnapshot().detectorModels?.occlusion[1]?.sha256, null);
  const invalid = modelStatus('yunet', 'faceocc');
  invalid.occlusion[1] = { provider: 'faceocc', state: 'invalid', sha256: 'c'.repeat(64), reason: 'Declared hash mismatch' };
  client.models = async () => invalid;
  await store.refreshDetectorModels();
  assert.deepEqual(store.getSnapshot().detectorModels?.occlusion[1], invalid.occlusion[1]);
  assert.equal(store.getSnapshot().settings?.occlusion_provider, 'faceocc');
  assert.equal(store.getSnapshot().settings?.occlusion_min_visible_fraction, 0.75);
});

test('per-eye visibility readings preserve server null eye state and independent eye measurements', async () => {
  const face: Face = {
    index: 0, bbox: [0, 0, 100, 100], confidence: 0.9, landmarks: [],
    left_eye: null, left_eye_unreliable_reason: 'occluded',
    right_eye: { state: 'open', ear: 0.2, blink_score: 0.1 },
    left_eye_visibility: { visible_fraction: 0.2, mean_probability: 0.3, sampled_pixels: 20, method: 'faceocc' },
    right_eye_visibility: { visible_fraction: 0.9, mean_probability: 0.8, sampled_pixels: 21, method: 'faceocc' },
  };
  const client = new IrisClient(new Transport(session, async () => Response.json({ ...photo(1), analysis: { faces: [face] } })));
  const received = (await client.photo(1)).analysis?.faces[0];
  assert.equal(received?.left_eye, null);
  assert.equal(received?.left_eye_unreliable_reason, 'occluded');
  assert.deepEqual(received?.left_eye_visibility, face.left_eye_visibility);
  assert.deepEqual(received?.right_eye_visibility, face.right_eye_visibility);
  assert.equal(received?.right_eye?.state, 'open');
});

test('keyboard crosses pages in both directions while retaining filters and sorting', async () => {
  const { store } = fixture(5); await store.openProject('C:/photos');
  const filter = { decision: 'pending' as const, format: 'jpeg', sort: 'name', descending: true, limit: 2 };
  await store.setFilter(filter);
  const commands = new CommandSystem(store);
  assert.equal(store.getSnapshot().focusedId, 5);
  await commands.dispatch('previous');
  assert.equal(store.getSnapshot().focusedId, 5);
  await Promise.all([commands.dispatch('next'), commands.dispatch('next')]);
  assert.equal(store.getSnapshot().focusedId, 3);
  assert.deepEqual(store.getSnapshot().filter, { ...filter, offset: 2 });
  await commands.dispatch('previous');
  assert.equal(store.getSnapshot().focusedId, 4);
  assert.equal(store.getSnapshot().filter.offset, 0);
  await Promise.all([commands.dispatch('next'), commands.dispatch('next'), commands.dispatch('next')]);
  assert.equal(store.getSnapshot().focusedId, 1);
  assert.equal(store.getSnapshot().filter.offset, 4);
  await commands.dispatch('next');
  assert.equal(store.getSnapshot().focusedId, 1);
  assert.equal(store.getSnapshot().filter.offset, 4);
});

test('navigation beyond an exactly full final page preserves the current page', async () => {
  const { store } = fixture(4); await store.openProject('C:/photos');
  await store.setFilter({ offset: 2, limit: 2 }); store.focus(4);
  await store.move(1);
  assert.deepEqual(store.getSnapshot().photos.map(photo => photo.id), [3, 4]);
  assert.equal(store.getSnapshot().focusedId, 4);
  assert.equal(store.getSnapshot().filter.offset, 2);
});

test('deciding the page end advances to the next page when the row remains visible', async () => {
  const { store } = fixture(5); await store.openProject('C:/photos');
  await store.setFilter({ sort: 'name', limit: 2 }); store.focus(2);
  await new CommandSystem(store).dispatch('keep');
  assert.equal(store.getSnapshot().focusedId, 3);
  assert.equal(store.getSnapshot().filter.offset, 2);
});

test('pending-only decisions continue at the shifted successor and retreat from an emptied last page', async () => {
  const { store } = fixture(5); await store.openProject('C:/photos');
  await store.setFilter({ decision: 'pending', sort: 'name', limit: 2 }); store.focus(2);
  await new CommandSystem(store).dispatch('keep');
  assert.deepEqual(store.getSnapshot().photos.map(photo => photo.id), [1, 3]);
  assert.equal(store.getSnapshot().focusedId, 3);
  assert.equal(store.getSnapshot().filter.offset, 0);
  await store.setFilter({ decision: 'pending', sort: 'name', offset: 2, limit: 2 });
  store.select([4, 5]); store.focus(5);
  await new CommandSystem(store).dispatch('reject');
  assert.deepEqual(store.getSnapshot().photos.map(photo => photo.id), [1, 3]);
  assert.equal(store.getSnapshot().focusedId, 3);
  assert.equal(store.getSnapshot().filter.offset, 0);
  assert.equal(store.getSnapshot().filter.decision, 'pending');
});

test('late page response cannot overwrite a new filter or a reopened project', async () => {
  for (const reopen of [false, true]) {
    const { store, client } = fixture(5); await store.openProject('C:/photos');
    await store.setFilter({ limit: 2 }); store.focus(2);
    let release!: (photos: Photo[]) => void;
    const read = client.photos;
    let started!: () => void;
    const startedPromise = new Promise<void>(resolve => { started = resolve; });
    client.photos = async () => new Promise(resolve => { release = resolve; started(); });
    const previous = store.move(1); await startedPromise;
    client.photos = read;
    if (reopen) await store.openProject('C:/photos');
    else await store.setFilter({ decision: 'pending', descending: true, limit: 2 });
    const current = store.getSnapshot();
    release([photo(3), photo(4)]); await previous;
    assert.deepEqual(store.getSnapshot().filter, current.filter);
    assert.deepEqual(store.getSnapshot().photos, current.photos);
    assert.equal(store.getSnapshot().focusedId, current.focusedId);
  }
});

test('late decision completion cannot advance a reopened project or execute its queued old-view decision', async () => {
  const { store, client, requests } = fixture(5); await store.openProject('C:/photos');
  let release!: () => void;
  let started!: () => void;
  const startedPromise = new Promise<void>(resolve => { started = resolve; });
  client.decide = async () => { await new Promise<void>(resolve => { release = resolve; started(); }); return { id: 'old', changed: 1, conflicts: [] }; };
  const first = store.decide('keep', true);
  const queued = store.decide('reject', true);
  await startedPromise;
  await store.openProject('C:/photos'); store.focus(4);
  release(); await Promise.all([first, queued]);
  assert.equal(store.getSnapshot().focusedId, 4);
  assert.equal(requests.filter(request => request.path.endsWith('/decisions')).length, 0);
  assert.equal(store.getSnapshot().pending, 0);
});

test('old progress-poll photos cannot replace a newly navigated page', async () => {
  const { store, client } = fixture(5); await store.openProject('C:/photos');
  await store.setFilter({ limit: 2 }); store.focus(2);
  const read = client.photos;
  let release!: (photos: Photo[]) => void;
  client.photos = async (_id, filter) => filter?.offset === 0 ? new Promise(resolve => { release = resolve; }) : read(_id, filter);
  const poll = store.refresh();
  await store.move(1);
  release([photo(1), photo(2)]); await poll;
  assert.deepEqual(store.getSnapshot().photos.map(photo => photo.id), [3, 4]);
  assert.equal(store.getSnapshot().focusedId, 3);
  assert.equal(store.getSnapshot().filter.offset, 2);
});

test('a filter change during a decision prevents its automatic navigation', async () => {
  const { store, client } = fixture(5); await store.openProject('C:/photos');
  await store.setFilter({ limit: 2 }); store.focus(2);
  const decide = client.decide;
  let release!: () => void;
  let started!: () => void;
  const startedPromise = new Promise<void>(resolve => { started = resolve; });
  client.decide = async (...args) => { await new Promise<void>(resolve => { release = resolve; started(); }); return decide(...args); };
  const previous = store.decide('keep', true); await startedPromise;
  await store.setFilter({ decision: 'pending', descending: true, limit: 2 });
  store.focus(4);
  release(); await previous;
  assert.equal(store.getSnapshot().focusedId, 4);
  assert.deepEqual(store.getSnapshot().filter, { decision: 'pending', descending: true, limit: 2, offset: 0 });
});

test('scoped acceptance captures only the visible page or selection and preserves empty IDs', async () => {
  const { store, requests } = fixture(5); await store.openProject('C:/photos');
  await store.setFilter({ decision: 'pending', offset: 2, limit: 2 });
  await store.acceptVisible('recommend');
  store.select([4]); await store.acceptSelected('reject_suggest');
  store.select([]); await store.acceptSelected();
  const explicit = { photo_ids: [3], category: 'recommend' as const };
  const pending = store.accept(explicit);
  explicit.photo_ids.push(1);
  await pending;
  await store.accept();
  assert.deepEqual(requests.filter(request => request.path.endsWith('/accept')).map(request => request.body), [
    { photo_ids: [3, 4], category: 'recommend' },
    { photo_ids: [4], category: 'reject_suggest' },
    { photo_ids: [], category: 'all' },
    { photo_ids: [3], category: 'recommend' },
    {},
  ]);
});

test('late scoped acceptance cannot refresh a reopened project or run queued old-view acceptance', async () => {
  const { store, client } = fixture(5); await store.openProject('C:/photos');
  let release!: () => void;
  let started!: () => void;
  const startedPromise = new Promise<void>(resolve => { started = resolve; });
  let writes = 0;
  client.accept = async () => { writes++; await new Promise<void>(resolve => { release = resolve; started(); }); return { id: 'old', changed: 1, conflicts: [] }; };
  const first = store.acceptVisible();
  const queued = store.accept({ photo_ids: [2] });
  await startedPromise;
  await store.openProject('C:/photos'); store.focus(4);
  let reads = 0;
  client.photos = async () => { reads++; return []; };
  await assert.rejects(store.execute(async () => { throw new Error('new project error'); }), /new project error/);
  release(); await Promise.all([first, queued]);
  assert.equal(writes, 1);
  assert.equal(reads, 0);
  assert.equal(store.getSnapshot().focusedId, 4);
  assert.equal(store.getSnapshot().pending, 0);
  assert.equal(store.getSnapshot().error, 'new project error');
});
