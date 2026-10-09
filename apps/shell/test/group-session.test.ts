import { test } from 'node:test';
import assert from 'node:assert/strict';
import { IrisClient, IrisStore, CommandSystem, Transport, defaultBindings, type Photo, type BurstGroup, type Socket } from '../src/index.js';

function fixture(linkVariants = false) {
  const project = { id: 1, root: 'C:/photos', name: 'test', cache_root: 'cache', created_at: '', groups_dirty: false, pending_analysis: 0, hidden: false, last_opened_at: '' };
  let photos: Photo[] = Array.from({ length: 12 }, (_, i) => ({ id: i + 1, project_id: 1, path: `${i}.jpg`, filename: `${i}.jpg`, format: 'jpeg', mtime: 1, size_bytes: 1, width: 1, height: 1, missing: false, quarantined: false, decision: 'pending', analysis_status: 'missing', capture_variant_id: linkVariants && [1, 8].includes(i + 1) ? 'same-capture' : null }));
  let groups: BurstGroup[] = [{ id: 'a', project_id: 1, kind: 'burst', member_photo_ids: [8, 3, 11] }, { id: 'b', project_id: 1, kind: 'burst', member_photo_ids: [9, 2] }];
  const requests: { path: string; body: Record<string, unknown> }[] = [];
  const client = new IrisClient(new Transport({ base_url: 'http://127.0.0.1:45678', token: 'test', version: '0.1.0' }, async (input, init) => {
    const url = new URL(String(input));
    const path = url.pathname.replace('/api/v1', '');
    const body = JSON.parse(String(init?.body ?? '{}')) as Record<string, unknown>;
    requests.push({ path, body });
    let value: unknown;
    if (path === '/projects' || path === '/projects/1') value = project;
    else if (path === '/settings') value = {};
    else if (path === '/models') value = { selected: 'yunet', detectors: [] };
    else if (path.endsWith('/photos')) value = photos.slice(0, 2);
    else if (path.startsWith('/photos/')) value = photos.find(photo => photo.id === Number(path.split('/').at(-1)));
    else if (path.endsWith('/groups')) value = groups;
    else if (path.endsWith('/progress')) value = { kind: 'analysis', state: 'completed', completed: 12, total: 12, errors: [], result: null };
    else if (path.endsWith('/quarantine')) value = [];
    else if (path.endsWith('/decisions')) {
      const ids = new Set(body.photo_ids as number[]);
      const variants = new Set(photos.filter(photo => ids.has(photo.id) && photo.capture_variant_id).map(photo => photo.capture_variant_id));
      photos = photos.map(photo => ids.has(photo.id) || body.link_variants && photo.capture_variant_id && variants.has(photo.capture_variant_id) && (photo.decision === 'pending' || photo.decision === body.action) ? { ...photo, decision: body.action as Photo['decision'] } : photo);
      value = { id: 'decision', changed: 1, conflicts: [] };
    } else if (path.endsWith('/accept')) value = { id: 'accept', changed: 0, conflicts: [] };
    else if (path.endsWith('/analyze')) value = { kind: 'analysis', state: 'running', completed: 0, total: 12, errors: [], result: null };
    else throw new Error(`Unexpected route ${path}`);
    return Response.json(value);
  }));
  return { client, store: new IrisStore(client), project, requests, setGroups: (value: BurstGroup[]) => { groups = value; } };
}

test('group loads all off-page members in server order and validates comparison candidates', async () => {
  const { store } = fixture(); await store.openProject('C:/photos');
  await store.openGroup('a');
  assert.deepEqual(store.getSnapshot().photos.map(photo => photo.id), [1, 2]);
  assert.deepEqual(store.getSnapshot().groupSession?.photos.map(photo => photo.id), [8, 3, 11]);
  assert.deepEqual(store.getSnapshot().groupSession?.comparisonIds, [8, 3]);
  assert.equal(store.getSnapshot().groupSession?.focusedId, 8);
  assert.equal(store.photoAnalysis(1)?.status, 'missing');
  assert.equal(store.photoAnalysis(8)?.status, 'missing');
  assert.equal(store.photoAnalysis(999), null);
  store.setComparisonCandidates([11, 8, 11]);
  store.focusGroupPhoto(11);
  assert.deepEqual(store.getSnapshot().groupSession?.comparisonIds, [11, 8]);
  assert.equal(store.getSnapshot().groupSession?.focusedId, 11);
  assert.throws(() => store.setComparisonCandidates([1]), /belong/);
  assert.throws(() => store.focusGroupPhoto(1), /member/);
  store.closeGroup();
  assert.equal(store.getSnapshot().groupSession, null);
  assert.equal(store.photoAnalysis(8), null);
});

test('group navigation and decisions advance within and between groups without changing library focus', async () => {
  const { store } = fixture(); await store.openProject('C:/photos');
  await store.openGroup('a');
  const commands = new CommandSystem(store);
  await commands.dispatch('previousGroup');
  assert.equal(store.getSnapshot().groupSession?.groupId, 'a');
  await commands.dispatch('nextGroupPhoto');
  assert.equal(store.getSnapshot().groupSession?.focusedId, 3);
  await commands.dispatch('groupKeep');
  assert.equal(store.getSnapshot().groupSession?.photos.find(photo => photo.id === 3)?.decision, 'keep');
  assert.equal(store.getSnapshot().groupSession?.focusedId, 11);
  await commands.dispatch('groupReject');
  assert.equal(store.getSnapshot().groupSession?.groupId, 'b');
  assert.equal(store.getSnapshot().groupSession?.focusedId, 9);
  await commands.dispatch('nextGroup');
  assert.equal(store.getSnapshot().groupSession?.groupId, 'b');
  await commands.dispatch('previousGroup');
  assert.equal(store.getSnapshot().groupSession?.groupId, 'a');
  assert.equal(store.getSnapshot().focusedId, 1);
});

async function press(commands: CommandSystem, key: string, modifiers: { ctrlKey?: boolean; metaKey?: boolean } = {}) {
  const errors: unknown[] = [];
  let prevented = false;
  assert.equal(commands.handle({ key, target: null, ...modifiers, preventDefault: () => { prevented = true; } } as unknown as KeyboardEvent, error => errors.push(error)), true);
  assert.equal(prevented, true);
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(errors, []);
}

test('default keyboard follows the group focus and advances between groups, then returns to the library', async () => {
  const { store, requests } = fixture(); await store.openProject('C:/photos');
  store.select([1, 2]);
  await store.openGroup('a');
  const commands = new CommandSystem(store);
  for (const key of ['d', 'ArrowRight', 'a', 'ArrowLeft']) await press(commands, key);
  assert.equal(store.getSnapshot().groupSession?.focusedId, 8);
  assert.equal(store.getSnapshot().focusedId, 1);
  await press(commands, 'p');
  assert.equal(store.getSnapshot().groupSession?.focusedId, 8);
  await press(commands, 'w');
  assert.equal(store.getSnapshot().groupSession?.focusedId, 3);
  await press(commands, 's');
  assert.equal(store.getSnapshot().groupSession?.focusedId, 11);
  await press(commands, 'ArrowUp');
  assert.equal(store.getSnapshot().groupSession?.groupId, 'b');
  assert.equal(store.getSnapshot().groupSession?.focusedId, 9);
  await press(commands, 'ArrowDown');
  assert.equal(store.getSnapshot().groupSession?.focusedId, 2);
  assert.deepEqual(requests.filter(request => request.path.endsWith('/decisions')).map(request => [request.body.photo_ids, request.body.action]), [
    [[8], 'flag'], [[8], 'keep'], [[3], 'reject'], [[11], 'keep'], [[9], 'reject'],
  ]);
  assert.equal(store.getSnapshot().focusedId, 1);
  store.closeGroup();
  store.select([]);
  await press(commands, 'd');
  assert.equal(store.getSnapshot().focusedId, 2);
  await press(commands, 'w');
  assert.deepEqual(requests.filter(request => request.path.endsWith('/decisions')).at(-1)?.body.photo_ids, [2]);
});

test('keyboard consumes actions during initial and refresh group loads without changing the library', async () => {
  for (const refresh of [false, true]) {
    const { store, client, requests } = fixture(); await store.openProject('C:/photos');
    if (refresh) await store.openGroup('a');
    const read = client.photo;
    let release!: () => void;
    const hold = new Promise<void>(resolve => { release = resolve; });
    client.photo = async id => { await hold; return read(id); };
    const loading = refresh ? store.refreshGroup() : store.openGroup('a');
    const commands = new CommandSystem(store);
    try {
      assert.equal(store.getSnapshot().groupSession?.loading, true);
      for (const key of ['w', 's', 'p', 'a', 'd', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight']) await press(commands, key);
      await press(commands, 'a', { ctrlKey: true });
      assert.equal(requests.filter(request => request.path.endsWith('/decisions')).length, 0);
      assert.equal(store.getSnapshot().focusedId, 1);
      assert.deepEqual(store.getSnapshot().selectedIds, []);
      assert.equal(store.getSnapshot().groupSession?.focusedId, 8);
    } finally { release(); await loading; }
    await press(commands, 'w');
    assert.deepEqual(requests.filter(request => request.path.endsWith('/decisions')).at(-1)?.body.photo_ids, [8]);
  }
});

test('custom keyboard commands use group context while explicit library dispatch remains compatible', async () => {
  const { store, requests } = fixture(); await store.openProject('C:/photos'); await store.openGroup('a');
  const commands = new CommandSystem(store, { ...defaultBindings, q: 'clear', Escape: 'closeGroup' });
  await press(commands, 'q');
  assert.deepEqual(requests.filter(request => request.path.endsWith('/decisions')).at(-1)?.body.photo_ids, [8]);
  assert.equal(requests.filter(request => request.path.endsWith('/decisions')).at(-1)?.body.action, 'pending');
  await press(commands, 'a', { ctrlKey: true });
  await press(commands, 'a', { metaKey: true });
  assert.deepEqual(store.getSnapshot().selectedIds, []);
  await commands.dispatch('keep');
  assert.deepEqual(requests.filter(request => request.path.endsWith('/decisions')).at(-1)?.body.photo_ids, [1]);
  assert.equal(store.getSnapshot().focusedId, 2);
  assert.equal(store.getSnapshot().groupSession?.focusedId, 8);
  await press(commands, 'Escape');
  assert.equal(store.getSnapshot().groupSession, null);
  await press(commands, 'a', { ctrlKey: true });
  assert.deepEqual(store.getSnapshot().selectedIds, [1, 2]);
});

test('a group keyboard decision queued before closing the group cannot fall back to library focus', async () => {
  const { store, requests } = fixture(); await store.openProject('C:/photos'); await store.openGroup('a');
  const commands = new CommandSystem(store);
  const handled = press(commands, 'w');
  store.closeGroup();
  await handled;
  assert.equal(requests.filter(request => request.path.endsWith('/decisions')).length, 0);
  assert.equal(store.getSnapshot().focusedId, 1);
});

test('group acceptance uses full group scope, not the library page or comparison candidates', async () => {
  const { store, requests } = fixture(); await store.openProject('C:/photos');
  await store.openGroup('a'); store.setComparisonCandidates([11]);
  await store.acceptGroup('recommend');
  assert.deepEqual(requests.find(request => request.path.endsWith('/accept'))?.body, { photo_ids: [8, 3, 11], category: 'recommend' });
  assert.deepEqual(store.getSnapshot().groupSession?.comparisonIds, [11]);
});

test('library decisions refresh linked off-page group members without an event connection', async () => {
  const { store, client, requests } = fixture(true);
  await store.openProject('C:/photos'); await store.openGroup('a');
  store.setComparisonCandidates([8, 11]); store.focusGroupPhoto(11);
  await store.decide('keep', true);
  assert.equal(requests.find(request => request.path.endsWith('/decisions'))?.body.link_variants, true);
  assert.equal((await client.photo(8)).decision, 'keep');
  assert.equal(store.getSnapshot().photos.find(photo => photo.id === 1)?.decision, 'keep');
  assert.equal(store.getSnapshot().groupSession?.photos.find(photo => photo.id === 8)?.decision, 'keep');
  assert.equal(store.getSnapshot().groupSession?.focusedId, 11);
  assert.deepEqual(store.getSnapshot().groupSession?.comparisonIds, [8, 11]);
  assert.equal(store.getSnapshot().focusedId, 2, 'library advancement still works after group refresh');
  assert.equal(store.getSnapshot().connection, 'disconnected');
});

test('late member reads cannot reopen a closed, switched, or reopened-project group', async () => {
  for (const action of ['close', 'group', 'project'] as const) {
    const { store, client } = fixture(); await store.openProject('C:/photos');
    const read = client.photo;
    let release!: () => void;
    const hold = new Promise<void>(resolve => { release = resolve; });
    client.photo = async id => { await hold; return read(id); };
    const previous = store.openGroup('a');
    client.photo = read;
    if (action === 'close') store.closeGroup();
    else if (action === 'group') await store.openGroup('b');
    else await store.openProject('C:/photos');
    release(); await previous;
    assert.equal(store.getSnapshot().groupSession?.groupId ?? null, action === 'group' ? 'b' : null);
    assert.equal(store.getSnapshot().pending, 0);
  }
});

test('group disappears on dirty analysis, changed membership, pending work, and explicit reanalysis', async () => {
  for (const reason of ['dirty', 'members', 'pending', 'reanalyze'] as const) {
    const { store, project, setGroups } = fixture(); await store.openProject('C:/photos'); await store.openGroup('a');
    if (reason === 'dirty') project.groups_dirty = true;
    else if (reason === 'pending') project.pending_analysis = 1;
    else if (reason === 'members') setGroups([{ id: 'a', project_id: 1, kind: 'burst', member_photo_ids: [11, 3, 8] }]);
    else await store.reanalyze();
    await store.refresh();
    assert.equal(store.getSnapshot().groupSession, null);
    assert.throws(() => store.acceptGroup(), /Open a current/);
  }
});

test('foreign or missing members fail closed and never expose a partial comparison', async () => {
  for (const foreign of [false, true]) {
    const { store, client } = fixture(); await store.openProject('C:/photos');
    const read = client.photo;
    client.photo = async id => ({ ...await read(id), ...(foreign ? { project_id: 2 } : { missing: true }) });
    await assert.rejects(store.openGroup('a'), /unavailable|another project/);
    assert.equal(store.getSnapshot().groupSession, null);
    assert.equal(store.getSnapshot().pending, 0);
  }
});

test('queued group acceptance is abandoned if the group closes before execution', async () => {
  const { store, requests } = fixture(); await store.openProject('C:/photos'); await store.openGroup('a');
  const operation = store.acceptGroup(); store.closeGroup(); await operation;
  assert.equal(requests.filter(request => request.path.endsWith('/accept')).length, 0);
});

test('member loading caps concurrency and rejects analysis that becomes stale during loading', async () => {
  const { store, client, project, setGroups } = fixture();
  setGroups([{ id: 'large', project_id: 1, kind: 'burst', member_photo_ids: Array.from({ length: 12 }, (_, i) => i + 1) }]);
  await store.openProject('C:/photos');
  const read = client.photo;
  let active = 0; let maximum = 0;
  client.photo = async id => {
    active++; maximum = Math.max(maximum, active);
    await new Promise(resolve => setImmediate(resolve));
    active--; return read(id);
  };
  await store.openGroup('large');
  assert.equal(store.getSnapshot().groupSession?.photos.length, 12);
  assert.ok(maximum <= 6 && maximum > 1);
  client.photo = async id => { project.groups_dirty = true; return read(id); };
  await store.refreshGroup();
  assert.equal(store.getSnapshot().groupSession, null);
});

test('a late failed group load cannot replace the new group error state', async () => {
  const { store, client } = fixture(); await store.openProject('C:/photos');
  const read = client.photo;
  let reject!: (reason: Error) => void;
  const hold = new Promise<Photo>((_resolve, fail) => { reject = fail; });
  client.photo = async () => hold;
  const previous = store.openGroup('a');
  client.photo = read;
  await store.openGroup('b');
  reject(new Error('old group failed')); await previous;
  assert.equal(store.getSnapshot().groupSession?.groupId, 'b');
  assert.equal(store.getSnapshot().error, null);
});

test('session events refresh off-page group decisions and analysis events invalidate the group', async () => {
  const { store, client } = fixture(); await store.openProject('C:/photos'); await store.openGroup('a');
  const socket: Socket = { onopen: null, onmessage: null, onerror: null, onclose: null, close() {} };
  store.connect({ createSocket: () => socket, pollMs: 60000 });
  try {
    await client.decide(1, [8], 'keep');
    const refreshed = new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => { unsubscribe(); reject(new Error('group event refresh timeout')); }, 2000);
      const unsubscribe = store.subscribe(() => {
        if (store.getSnapshot().groupSession?.photos.find(photo => photo.id === 8)?.decision === 'keep') {
          clearTimeout(timer); unsubscribe(); resolve();
        }
      });
    });
    socket.onmessage?.({ data: JSON.stringify({ event: 'session:changed', project_id: 1 }) } as MessageEvent);
    await refreshed;
    socket.onmessage?.({ data: JSON.stringify({ event: 'analysis:stage', project_id: 1, data: { kind: 'analysis', state: 'running' } }) } as MessageEvent);
    assert.equal(store.getSnapshot().groupSession, null);
  } finally { store.dispose(); }
});

test('HTTP fallback refreshes off-page decisions while the event socket is disconnected', async () => {
  const { store, client } = fixture(); await store.openProject('C:/photos'); await store.openGroup('a');
  await client.decide(1, [11], 'reject');
  const socket: Socket = { onopen: null, onmessage: null, onerror: null, onclose: null, close() {} };
  const refreshed = new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => { unsubscribe(); reject(new Error('group poll refresh timeout')); }, 2000);
    const unsubscribe = store.subscribe(() => {
      if (store.getSnapshot().groupSession?.photos.find(photo => photo.id === 11)?.decision === 'reject') {
        clearTimeout(timer); unsubscribe(); resolve();
      }
    });
  });
  store.connect({ createSocket: () => socket, pollMs: 10 });
  try { await refreshed; } finally { store.dispose(); }
});
