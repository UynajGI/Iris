import test from 'node:test';
import assert from 'node:assert/strict';
import {
  EditingDrafts,
  resolveDraft,
  hasUnsavedDraft,
  mergeSavedDraft,
  type DraftStorage,
  type EditDraft,
} from '../src/editing-drafts.js';
import type { EditDocument, EditRecord } from '../src/editing-client.js';
import type { EditingClient } from '../src/editing-client.js';
import { loadEditingDraft, recoverDraftCopy } from '../src/editing-draft-recovery.js';

const doc = (output = 'source'): EditDocument => ({
  schema_version: 1,
  source: {
    photo_id: 'photo',
    fingerprint: 'sha',
    width: 1,
    height: 1,
    reference_kind: 'scene',
  },
  nodes: [],
  panel_bindings: [],
  node_positions: {},
  output_node: output,
  output: {} as EditDocument['output'],
});
const base: EditRecord = {
  id: 'edit',
  photo_id: 1,
  revision: 3,
  document: doc(),
  name: 'copy',
  created_at: '',
  updated_at: '',
};
const pending = {
  record: base,
  document: doc('saved'),
  requestId: '6ec518e8-7688-4ee6-820a-33dcf952a3da',
};
function storage(): DraftStorage & { values: Map<string, string> } {
  const values = new Map<string, string>();
  return {
    values,
    getItem: (k) => values.get(k) ?? null,
    setItem: (k, v) => {
      values.set(k, v);
    },
    removeItem: (k) => {
      values.delete(k);
    },
  };
}
const snapshot = (document = doc('draft')): EditDraft => ({
  format: 1,
  record: base,
  document,
  pending: null,
});

test('draft and pending receipt survive flush and a fresh store instance', () => {
  const disk = storage(),
    first = new EditingDrafts(disk);
  first.write(base, doc('draft'), pending);
  assert.equal(disk.values.size, 0);
  assert.equal(first.flush(), true);
  assert.deepEqual(new EditingDrafts(disk).read(base.id), { ...snapshot(), pending });
  first.remove(base.id);
  assert.equal(new EditingDrafts(disk).read(base.id), null);
});
test('coalesced changes retain the latest document and clean saves remove it', () => {
  const disk = storage(),
    store = new EditingDrafts(disk);
  for (let i = 0; i < 100; i++) store.write(base, doc(String(i)));
  store.flush();
  assert.equal(new EditingDrafts(disk).read(base.id)?.document.output_node, '99');
  store.write(base, base.document);
  store.flush();
  assert.equal(disk.values.size, 0);
});
test('matching revisions recover; changed revisions or source identity conflict', () => {
  assert.equal(resolveDraft(base, snapshot()).kind, 'restore');
  assert.equal(resolveDraft({ ...base, revision: 4 }, snapshot()).kind, 'conflict');
  assert.equal(resolveDraft({ ...base, id: 'other' }, snapshot()).kind, 'conflict');
  assert.equal(
    resolveDraft(
      {
        ...base,
        document: { ...base.document, source: { ...base.document.source, fingerprint: 'changed' } },
      },
      snapshot()
    ).kind,
    'conflict'
  );
  assert.equal(resolveDraft(base, snapshot(base.document)).kind, 'clean');
});
test('a committed request with a lost response keeps newer unsaved input without replay', () => {
  const current = { ...base, revision: 4, document: pending.document };
  assert.deepEqual(resolveDraft(current, { ...snapshot(), pending }), {
    kind: 'restore',
    document: doc('draft'),
    pending: null,
  });
  assert.equal(
    resolveDraft({ ...current, revision: 5 }, { ...snapshot(), pending }).kind,
    'conflict'
  );
  assert.equal(resolveDraft(current, { ...snapshot(pending.document), pending }).kind, 'clean');
});
test('storage quota failures retain in-memory changes and the last durable copy', () => {
  const disk = storage(),
    warnings: boolean[] = [],
    store = new EditingDrafts(disk, (x) => warnings.push(x));
  store.write(base, doc('old'));
  store.flush();
  disk.setItem = () => {
    throw new Error('quota');
  };
  store.write(base, doc('latest'));
  assert.equal(store.flush(), false);
  assert.equal(store.read(base.id)?.document.output_node, 'latest');
  assert.equal(new EditingDrafts(disk).read(base.id)?.document.output_node, 'old');
  assert.equal(warnings.at(-1), true);
});
test('malformed storage never enters the editor as a document', () => {
  const disk = storage();
  disk.values.set('iris.editing-draft.v1.edit', '{"format":1}');
  assert.throws(() => new EditingDrafts(disk).read('edit'), /格式损坏/);
  disk.values.set('iris.editing-draft.v1.edit', '{');
  assert.throws(() => new EditingDrafts(disk).read('edit'));
});

test('reverted input still retains an in-flight save receipt until it is reconciled', () => {
  assert.deepEqual(resolveDraft(base, { ...snapshot(base.document), pending }), {
    kind: 'restore',
    document: base.document,
    pending,
  });
});

test('save responses normalize the submitted draft without replacing newer input', () => {
  const submitted = doc('submitted'),
    saved = doc('canonical'),
    newer = doc('newer');
  assert.equal(mergeSavedDraft(structuredClone(submitted), submitted, saved), saved);
  assert.equal(mergeSavedDraft(newer, submitted, saved), newer);
  assert.equal(mergeSavedDraft(null, submitted, saved), null);
});

test('pending receipts and unresolved recovery retain dirty protection', () => {
  assert.equal(hasUnsavedDraft(base, base.document, null, false), false);
  assert.equal(hasUnsavedDraft(base, base.document, pending, false), true);
  assert.equal(hasUnsavedDraft(base, base.document, null, true), true);
  assert.equal(hasUnsavedDraft(null, null, pending, false), false);
});

function apiStub(current = base) {
  const calls: string[] = [];
  const api = {
    preview: async () => {
      calls.push('validate');
      return new Blob();
    },
    close: async () => {
      calls.push('close');
    },
    get: async () => {
      calls.push('get');
      return current;
    },
    duplicate: async () => {
      calls.push('duplicate');
      return { ...current, id: 'recovered', revision: 0 };
    },
  };
  return { api: api as unknown as EditingClient, calls, implementation: api };
}
test('recovery validates the persisted graph before returning editable controls', async () => {
  const store = new EditingDrafts(storage());
  store.write(base, doc('draft'));
  store.flush();
  const { api, calls } = apiStub();
  const restored = await loadEditingDraft(api, store, base, new AbortController().signal);
  assert.equal(restored.document.output_node, 'draft');
  assert.equal(restored.recovery, null);
  assert.deepEqual(calls, ['validate', 'close']);
});
test('a rejected or unavailable draft remains recoverable and is never exposed as controls', async () => {
  const store = new EditingDrafts(storage());
  store.write(base, doc('draft'));
  store.flush();
  const { api, implementation } = apiStub();
  implementation.preview = async () => {
    throw new Error('Invalid graph');
  };
  const restored = await loadEditingDraft(api, store, base, new AbortController().signal);
  assert.equal(restored.document, base.document);
  assert.match(restored.recovery!.message, /Invalid graph/);
  assert.equal(store.read(base.id)?.document.output_node, 'draft');
});
test('conflicts never overwrite newer edits and restore only to a separately created copy', async () => {
  const store = new EditingDrafts(storage());
  store.write(base, doc('draft'));
  store.flush();
  const current = { ...base, revision: 4, document: doc('other-window') };
  const { api, calls } = apiStub(current);
  const loaded = await loadEditingDraft(api, store, current, new AbortController().signal);
  assert.equal(loaded.recovery!.conflict, true);
  assert.equal(loaded.document, current.document);
  assert.deepEqual(calls, []);
  const recovered = await recoverDraftCopy(api, loaded.recovery!, new AbortController().signal);
  assert.equal(recovered.record.id, 'recovered');
  assert.equal(recovered.document.output_node, 'draft');
  assert.equal(current.document.output_node, 'other-window');
  assert.deepEqual(calls, ['get', 'validate', 'close', 'duplicate']);
});
test('aborted recovery cannot publish a recovered draft', async () => {
  const store = new EditingDrafts(storage());
  store.write(base, doc('draft'));
  store.flush();
  const { api } = apiStub();
  const controller = new AbortController();
  controller.abort();
  await assert.rejects(loadEditingDraft(api, store, base, controller.signal), {
    name: 'AbortError',
  });
});
