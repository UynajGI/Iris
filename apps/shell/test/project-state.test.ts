import { test } from 'node:test';
import assert from 'node:assert/strict';
import { IrisClient, IrisStore, Transport, ViewPreferencesRepository, type CacheMigration } from '../src/index.js';

function fixture() {
  let opened = 0;
  const projects = [1, 2].map(id => ({ id, root: `C:/project${id}`, name: `project${id}`, created_at: '', cache_root: `C:/cache${id}`, groups_dirty: false, pending_analysis: 0, last_opened_at: String(id), hidden: false }));
  let migrations: CacheMigration[] = [];
  const requests: { path: string; method: string; body: Record<string, unknown> }[] = [];
  const client = new IrisClient(new Transport({ base_url: 'http://127.0.0.1:45678', token: 'secret', version: '0.1.0' }, async (input, init) => {
    const path = new URL(String(input)).pathname.replace('/api/v1', '');
    const method = init?.method ?? 'GET';
    const body = JSON.parse(String(init?.body ?? '{}')) as Record<string, unknown>;
    requests.push({ path, method, body });
    const project = projects[Number(path.match(/^\/projects\/(\d+)/)?.[1] ?? 1) - 1]!;
    let value: unknown;
    if (path === '/bootstrap') value = { version: '0.1.0', capabilities: ['jpeg'] };
    else if (path === '/profiles') value = [];
    else if (path === '/projects' && method === 'GET') value = projects.filter(p => !p.hidden).sort((a, b) => b.last_opened_at.localeCompare(a.last_opened_at));
    else if (path === '/projects') {
      const selected = projects.find(project => project.root === body.root)!;
      selected.last_opened_at = String(++opened + 10); selected.hidden = false; value = selected;
    } else if (path.endsWith('/open')) { project.last_opened_at = String(++opened + 10); project.hidden = false; value = project; }
    else if (path.endsWith('/hide')) { project.hidden = true; value = project; }
    else if (/^\/projects\/\d+$/.test(path)) value = project;
    else if (path === '/settings') value = {};
    else if (path === '/models') value = { selected: 'yunet', detectors: [] };
    else if (path.endsWith('/photos') || path.endsWith('/groups') || path.endsWith('/quarantine')) value = [];
    else if (path.endsWith('/progress')) value = { kind: 'analysis', state: 'completed', completed: 0, total: 0, errors: [], result: null };
    else if (path.endsWith('/cache/migrate')) {
      migrations.push({ id: 'migration-1', project_id: project.id, source_root: project.cache_root, destination_root: body.destination as string, created_at: '', state: 'ready', files: [{ path: 'thumb.jpg', size_bytes: 10, sha256: 'a'.repeat(64), cleaned: false }], error: null });
      project.cache_root = body.destination as string; value = { root: project.cache_root, files: 1, bytes: 10 };
    } else if (path.endsWith('/cache/migrations')) value = migrations.filter(migration => migration.project_id === project.id);
    else if (path.includes('/cache/migrations/') && path.endsWith('/cleanup')) {
      migrations = migrations.map(migration => ({ ...migration, state: 'cleaned', files: migration.files.map(file => ({ ...file, cleaned: true })) }));
      value = migrations[0];
    } else if (path.endsWith('/cache')) value = { root: project.cache_root, files: 1, bytes: 10 };
    else throw new Error(`Unexpected route ${path}`);
    return Response.json(value);
  }));
  const preferences = new ViewPreferencesRepository();
  return { client, store: new IrisStore(client, preferences), preferences, requests, projects };
}

test('project switches and daemon restoration preserve per-project preferences without recording a new open', async () => {
  const { store, client, preferences, requests } = fixture();
  await store.initialize(); await store.openProject('C:/project1');
  await store.setFilter({ sort: 'size', descending: true, decision: 'pending', offset: 50, limit: 50 });
  store.toggleReview();
  await store.openProject('C:/project2');
  assert.deepEqual(store.getSnapshot().filter, { limit: 200, offset: 0 });
  assert.equal(store.getSnapshot().reviewOpen, false);
  await store.setFilter({ sort: 'date' });
  await store.openProject('C:/project1');
  assert.deepEqual(store.getSnapshot().filter, { limit: 50, sort: 'size', descending: true, decision: 'pending', offset: 0 });
  assert.equal(store.getSnapshot().reviewOpen, true);
  const opens = requests.filter(request => request.method === 'POST').length;
  const restored = new IrisStore(client, preferences);
  await restored.initialize(); await restored.restoreProject(1); await restored.refresh(); await restored.refresh();
  assert.equal(requests.filter(request => request.method === 'POST').length, opens);
  assert.equal(restored.getSnapshot().filter.sort, 'size');
  assert.equal(restored.getSnapshot().reviewOpen, true);
  assert.equal(restored.getSnapshot().projects[0]?.id, 1);
});

test('recent hide affects only the list and explicit open unhides and reorders it', async () => {
  const { store, requests } = fixture(); await store.initialize();
  assert.deepEqual(store.getSnapshot().projects.map(project => project.id), [2, 1]);
  await store.openRecentProject(1);
  assert.deepEqual(store.getSnapshot().projects.map(project => project.id), [1, 2]);
  await store.hideRecentProject(1);
  assert.equal(store.getSnapshot().project?.id, 1);
  assert.deepEqual(store.getSnapshot().projects.map(project => project.id), [2]);
  await store.refresh();
  assert.deepEqual(store.getSnapshot().projects.map(project => project.id), [2]);
  await store.openRecentProject(1);
  assert.equal(store.getSnapshot().projects[0]?.hidden, false);
  assert.equal(requests.filter(request => request.path.endsWith('/open')).length, 2);
});

test('cache migration records stay separate from cleanup and require exact reviewed ID', async () => {
  const { store, requests } = fixture(); await store.openProject('C:/project1');
  await store.migrateCache('C:/new-cache');
  assert.equal(store.getSnapshot().cache?.root, 'C:/new-cache');
  assert.equal(store.getSnapshot().cacheMigrations[0]?.state, 'ready');
  assert.equal(requests.filter(request => request.path.endsWith('/cleanup')).length, 0);
  await assert.rejects(store.cleanupPreviousCache('migration-1'), /Review and confirm/);
  assert.throws(() => store.reviewCacheMigration('unknown'), /current project history/);
  store.reviewCacheMigration('migration-1');
  await assert.rejects(store.cleanupPreviousCache('wrong'), /Review and confirm/);
  await store.cleanupPreviousCache('migration-1');
  assert.equal(store.getSnapshot().cacheMigration?.state, 'cleaned');
  assert.equal(requests.filter(request => request.path.endsWith('/cleanup')).length, 1);
});

test('late cache history cannot repopulate a different active project', async () => {
  const { store, client } = fixture(); await store.openProject('C:/project1'); await store.migrateCache('C:/new-cache');
  const old = store.getSnapshot().cacheMigrations;
  let release!: (value: CacheMigration[]) => void;
  client.cacheMigrations = async () => new Promise(resolve => { release = resolve; });
  const previous = store.refreshCache();
  await store.openProject('C:/project2');
  release(old); await previous;
  assert.equal(store.getSnapshot().project?.id, 2);
  assert.equal(store.getSnapshot().cache, null);
  assert.deepEqual(store.getSnapshot().cacheMigrations, []);
});

test('failed old-cache cleanup refreshes its recorded recovery status and keeps the original error', async () => {
  const { store, client } = fixture(); await store.openProject('C:/project1'); await store.migrateCache('C:/new-cache');
  store.reviewCacheMigration('migration-1');
  const recorded = store.getSnapshot().cacheMigration!;
  client.cleanupPreviousCache = async () => { throw new Error('old cache file changed'); };
  client.cacheMigrations = async () => [{ ...recorded, state: 'cleanup_in_progress', error: 'old cache file changed' }];
  await assert.rejects(store.cleanupPreviousCache(recorded.id), /old cache file changed/);
  assert.equal(store.getSnapshot().cacheMigration?.state, 'cleanup_in_progress');
  assert.equal(store.getSnapshot().cacheMigration?.error, 'old cache file changed');
  assert.equal(store.getSnapshot().error, 'old cache file changed');
});
