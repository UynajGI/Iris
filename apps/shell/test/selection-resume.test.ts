import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  IrisClient,
  IrisStore,
  Transport,
  ViewPreferencesRepository,
  type Photo,
  type BurstGroup,
} from '../src/index.js';

function fixture() {
  const storage = new Map<string, string>();
  let writes = 0;
  const repository = () =>
    new ViewPreferencesRepository({
      getItem: (key) => storage.get(key) ?? null,
      setItem: (key, value) => {
        storage.set(key, value);
        ++writes;
      },
    });
  const projects = [1, 2].map((id) => ({
    id,
    root: `C:/public-fixture-${id}`,
    name: 'fixture',
    created_at: '',
    cache_root: 'cache',
    groups_dirty: false,
    pending_analysis: 0,
    hidden: false,
    last_opened_at: '',
  }));
  let scenes = ['scene-1'];
  let groups: BurstGroup[] = [
    { id: 'group-1', project_id: 1, kind: 'burst', member_photo_ids: [1, 3, 6] },
  ];
  const requests: string[] = [];
  const client = new IrisClient(
    new Transport(
      { base_url: 'http://127.0.0.1:45678', token: 'synthetic', version: '0.1.0' },
      async (input, init) => {
        const url = new URL(String(input));
        const path = url.pathname.replace('/api/v1', '');
        requests.push(path + url.search);
        const pid = Number(path.match(/^\/projects\/(\d+)/)?.[1] ?? 1);
        const project = projects[pid - 1]!;
        if (path === '/projects') {
          const body = JSON.parse(String(init?.body ?? '{}'));
          return Response.json(projects.find((p) => p.root === body.root));
        }
        if (/^\/projects\/\d+(\/open)?$/.test(path)) return Response.json(project);
        if (path.startsWith('/photos/'))
          return Response.json(photo(Number(path.split('/').at(-1)), 1));
        const resources: Record<string, unknown> = {
          settings: {},
          models: { selected: 'yunet', detectors: [] },
          groups: pid === 1 ? groups : [],
          scenes: scenes.map((id) => ({ id, member_photo_ids: [1, 2, 3, 4] })),
          quarantine: [],
          progress: {
            kind: 'analysis',
            state: 'completed',
            completed: 8,
            total: 8,
            errors: [],
            result: null,
          },
        };
        const resource = path.split('/').at(-1)!;
        if (resource in resources) return Response.json(resources[resource]);
        if (path.endsWith('/photos')) {
          return Response.json(filteredPhotos(pid, url.searchParams, scenes));
        }
        throw new Error(`Unexpected route ${path}`);
      }
    )
  );
  return {
    client,
    projects,
    repository,
    storage,
    requests,
    writes: () => writes,
    store: () => new IrisStore(client, repository()),
    removeScenes: () => {
      scenes = [];
    },
    groups: (value: BurstGroup[]) => {
      groups = value;
    },
  };
}

function filteredPhotos(pid: number, q: URLSearchParams, scenes: string[]) {
  let photos = Array.from({ length: 8 }, (_, i) => photo(pid === 1 ? i + 1 : i + 101, pid));
  if (q.has('scene_id')) photos = scenes.includes(q.get('scene_id')!) ? photos.slice(0, 4) : [];
  if (q.has('photo_id')) photos = photos.filter((p) => p.id === Number(q.get('photo_id')));
  if (q.get('descending') === 'true') photos.reverse();
  const offset = Number(q.get('offset') ?? 0),
    limit = Number(q.get('limit') ?? 200);
  return photos.slice(offset, offset + limit);
}

function photo(id: number, project_id: number): Photo {
  return {
    id,
    project_id,
    path: `${id}.jpg`,
    filename: `${id}.jpg`,
    format: 'jpeg',
    mtime: 1,
    size_bytes: 1,
    width: 32,
    height: 32,
    missing: false,
    quarantined: false,
    decision: 'pending',
    analysis_status: 'missing',
    rating: 0,
    color_label: 'none',
  };
}

test('a fresh store restores the page, focused photo and loupe without restoring a dangerous batch selection', async () => {
  const f = fixture();
  const first = f.store();
  await first.openProject(f.projects[0]!.root);
  await first.setFilter({ limit: 2, offset: 4, sort: 'date' });
  first.focus(6);
  first.toggleReview();
  first.select([5, 6]);
  const restored = f.store();
  await restored.restoreProject(1);
  assert.equal(restored.getSnapshot().focusedId, 6);
  assert.equal(restored.getSnapshot().filter.offset, 4);
  assert.equal(restored.getSnapshot().reviewOpen, true);
  assert.deepEqual(restored.getSnapshot().selectedIds, []);
  assert.deepEqual(
    restored.getSnapshot().photos.map((p) => p.id),
    [5, 6]
  );
});

test('scene scope and its page survive restart and unavailable scene scope falls back to the library', async () => {
  const f = fixture(),
    first = f.store();
  await first.openProject(f.projects[0]!.root);
  await first.setFilter({ scene_id: 'scene-1', limit: 2, offset: 2 });
  first.focus(4);
  const resumed = f.store();
  await resumed.openProject(f.projects[0]!.root);
  assert.equal(resumed.getSnapshot().filter.scene_id, 'scene-1');
  assert.equal(resumed.getSnapshot().focusedId, 4);
  f.removeScenes();
  const recovered = f.store();
  await recovered.openProject(f.projects[0]!.root);
  assert.equal(recovered.getSnapshot().filter.scene_id, undefined);
  assert.equal(recovered.getSnapshot().filter.offset, 0);
  assert.deepEqual(
    recovered.getSnapshot().photos.map((p) => p.id),
    [1, 2]
  );
});

test('comparison members and group focus restore independently of the library page and project', async () => {
  const f = fixture(),
    first = f.store();
  await first.openProject(f.projects[0]!.root);
  first.focus(2);
  await first.openGroup('group-1');
  first.setComparisonCandidates([3, 6]);
  first.focusGroupPhoto(6);
  await first.openProject(f.projects[1]!.root);
  assert.equal(first.getSnapshot().groupSession, null);
  await first.openRecentProject(1);
  assert.deepEqual(first.getSnapshot().groupSession?.comparisonIds, [3, 6]);
  assert.equal(first.getSnapshot().groupSession?.focusedId, 6);
  assert.equal(first.getSnapshot().focusedId, 2);
  const restarted = f.store();
  await restarted.restoreProject(1);
  assert.deepEqual(restarted.getSnapshot().groupSession?.comparisonIds, [3, 6]);
});

test('restarted comparison discards removed group members and dirty analysis does not reopen it', async () => {
  const f = fixture(),
    first = f.store();
  await first.openProject(f.projects[0]!.root);
  await first.openGroup('group-1');
  first.setComparisonCandidates([3, 6]);
  f.groups([{ id: 'group-1', project_id: 1, kind: 'burst', member_photo_ids: [1, 8] }]);
  const resumed = f.store();
  await resumed.restoreProject(1);
  assert.deepEqual(resumed.getSnapshot().groupSession?.comparisonIds, []);
  assert.equal(resumed.getSnapshot().groupSession?.focusedId, null);
  f.projects[0]!.groups_dirty = true;
  const stale = f.store();
  await stale.restoreProject(1);
  assert.equal(stale.getSnapshot().groupSession, null);
});

test('stale single-photo scopes and out-of-range pages recover without discarding user filters', async () => {
  const f = fixture();
  f.repository().save(f.projects[0]!.root, {
    filter: { descending: true, limit: 2 },
    reviewOpen: true,
    resume: { filter: { descending: true, limit: 2, offset: 100, photo_id: 999 }, focusedId: 999 },
  });
  const resumed = f.store();
  await resumed.restoreProject(1);
  assert.deepEqual(resumed.getSnapshot().filter, { limit: 2, descending: true, offset: 0 });
  assert.equal(resumed.getSnapshot().focusedId, 8);
});

test('a late scene validation cannot overwrite another project or its saved workspace', async () => {
  const f = fixture();
  f.repository().save(f.projects[0]!.root, {
    filter: {},
    reviewOpen: true,
    resume: { filter: { scene_id: 'scene-1' }, focusedId: 3 },
  });
  const readScenes = f.client.scenes;
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  let started!: () => void;
  const reached = new Promise<void>((resolve) => {
    started = resolve;
  });
  f.client.scenes = async (id) => {
    started();
    await gate;
    return readScenes(id);
  };
  const store = f.store();
  const old = store.openProject(f.projects[0]!.root);
  await reached;
  await store.openProject(f.projects[1]!.root);
  store.focus(104);
  release();
  await old;
  assert.equal(store.getSnapshot().project?.id, 2);
  assert.equal(store.getSnapshot().focusedId, 104);
  assert.equal(f.repository().load(f.projects[0]!.root).resume?.filter.scene_id, 'scene-1');
  assert.equal(f.repository().load(f.projects[1]!.root).resume?.focusedId, 104);
});

test('resume storage bounds candidates, rejects malformed scopes and never persists unrelated fields', () => {
  const f = fixture(),
    repository = f.repository();
  repository.save('root', {
    filter: {},
    reviewOpen: false,
    resume: {
      filter: { limit: 20, offset: 40, scene_id: 'scene-1', token: 'secret' } as never,
      focusedId: 2,
      group: {
        groupId: 'group-1',
        focusedId: 2,
        comparisonIds: Array.from({ length: 60 }, (_, i) => i + 1),
      },
      mediaUrl: 'blob:private',
    } as never,
  });
  const resume = f.repository().load('root').resume!;
  assert.equal(resume.group?.comparisonIds.length, 32);
  assert.equal(resume.filter.offset, 40);
  assert.ok(!/secret|blob:|mediaUrl|token/.test([...f.storage.values()].join('')));
  repository.save('root', {
    filter: {},
    reviewOpen: false,
    resume: { filter: { offset: -1 }, focusedId: 2 },
  });
  assert.equal(f.repository().load('root').resume, undefined);
});

test('unchanged focus and polling do not repeatedly write browser storage', async () => {
  const f = fixture(),
    store = f.store();
  await store.openProject(f.projects[0]!.root);
  store.focus(2);
  const writes = f.writes();
  store.focus(2);
  await store.refresh();
  await store.refresh();
  assert.equal(f.writes(), writes);
});
