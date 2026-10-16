import { test } from 'node:test';
import assert from 'node:assert/strict';
import { EditingClient, type EditRecord } from '../src/editing-client.js';
import {
  defaultSyncGroups,
  loadSyncCandidates,
  prepareSyncTargets,
  syncRequest,
  type SyncCandidate,
} from '../src/editing-sync.js';
import type { Photo } from '../src/types.js';
import { ApiError } from '../src/api-error.js';
import { syncErrorMessage } from '../src/editing-sync.js';

const photo = (id: number) => ({ id, filename: `${id}.png` }) as Photo;

test('sync errors distinguish definite conflicts from uncertain server results', () => {
  assert.match(
    syncErrorMessage(new ApiError(400, { error: 'edit revision conflict' })),
    /本次未同步/
  );
  assert.match(syncErrorMessage(new ApiError(503, 'service unavailable')), /无法确认同步结果/);
  assert.match(
    syncErrorMessage(new ApiError(400, 'enable target basic panel before selective sync')),
    /启用面板/
  );
});
const record = (id: string, photo_id: number, revision = 0) =>
  ({ id, photo_id, revision }) as EditRecord;
const row = (id: number, versions: EditRecord[], selected = true): SyncCandidate => ({
  photo: photo(id),
  versions,
  selected,
  editId: versions.length === 1 ? versions[0]!.id : '',
});

test('sync catalog reads use bounded concurrency, preserve order, and never choose among virtual copies', async () => {
  let active = 0,
    peak = 0;
  const api = {
    list: async (id: number) => {
      peak = Math.max(peak, ++active);
      await new Promise((resolve) => setTimeout(resolve, id % 3));
      active--;
      return id === 1 ? [record('a', 1), record('b', 1)] : id === 2 ? [] : [record(String(id), id)];
    },
  } as unknown as EditingClient;
  const rows = await loadSyncCandidates(
    api,
    Array.from({ length: 12 }, (_, i) => photo(i)),
    new AbortController().signal
  );
  assert.equal(peak, 4);
  assert.deepEqual(
    rows.map((r) => r.photo.id),
    Array.from({ length: 12 }, (_, i) => i)
  );
  assert.equal(rows[1]!.editId, '');
  assert.equal(rows[2]!.editId, '');
  assert.ok(rows.every((r) => !r.selected));
});

test('sync validates all version selections before creating any initial edits', async () => {
  const api = {
    create: () => {
      throw new Error('must not create');
    },
  } as unknown as EditingClient;
  await assert.rejects(
    prepareSyncTargets(api, [row(1, []), row(2, [record('a', 2), record('b', 2)])]),
    /选择目标版本/
  );
});

test('sync prepares only selected untouched photos and retains displayed target revisions', async () => {
  const created: number[] = [];
  const api = {
    list: async () => [],
    create: async (id: number) => {
      created.push(id);
      return record('new', id);
    },
  } as unknown as EditingClient;
  const existing = record('existing', 3, 7);
  const targets = await prepareSyncTargets(api, [
    row(1, [], false),
    row(2, []),
    row(3, [existing]),
  ]);
  assert.deepEqual(created, [2]);
  assert.deepEqual(targets, [record('new', 2), existing]);
  const request = syncRequest(record('source', 9, 4), targets, ['tone'], 'retry-id');
  assert.deepEqual(request, {
    expected_revision: 4,
    request_id: 'retry-id',
    targets: [
      { id: 'new', expected_revision: 0 },
      { id: 'existing', expected_revision: 7 },
    ],
    groups: ['tone'],
  });
});

test('a newly appeared edit requires refreshing rather than guessing a target', async () => {
  const api = {
    list: async () => [record('other-window', 2)],
    create: () => {
      throw new Error('must not create');
    },
  } as unknown as EditingClient;
  await assert.rejects(prepareSyncTargets(api, [row(2, [])]), /刷新后选择/);
});

test('sync defaults preserve crop and optics, rejects source and duplicate targets', () => {
  assert.ok(!defaultSyncGroups.includes('crop'));
  assert.ok(!defaultSyncGroups.includes('optics'));
  const source = record('source', 1, 3);
  assert.throws(() => syncRequest(source, [], ['tone'], 'r'));
  assert.throws(() => syncRequest(source, [record('other-copy', 1)], ['tone'], 'r'));
  assert.throws(() => syncRequest(source, [record('a', 2), record('a', 2)], ['tone'], 'r'));
});

test('editing client sends the exact sync snapshot to the shared service', async () => {
  const calls: unknown[][] = [];
  const client = new EditingClient({
    transport: {
      request: async (...args: unknown[]) => {
        calls.push(args);
        return [];
      },
    },
  } as never);
  const request = syncRequest(
    record('source', 1, 5),
    [record('target', 2, 9)],
    ['tone', 'curves'],
    'receipt'
  );
  await client.sync('edit/1', request);
  assert.deepEqual(calls, [['POST', '/api/v1/edits/edit%2F1/sync', request]]);
});
