import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ViewPreferencesRepository, validatedPhotoFilter, type PreferenceStorage } from '../src/index.js';

class MemoryStorage implements PreferenceStorage {
  values = new Map<string, string>();
  getItem(key: string) { return this.values.get(key) ?? null; }
  setItem(key: string, value: string) { this.values.set(key, value); }
}

test('project preferences persist only allowed fields and isolate canonical roots', () => {
  const storage = new MemoryStorage();
  const preferences = new ViewPreferencesRepository(storage);
  preferences.save('C:/照片 A', { filter: { sort: 'size', descending: true, decision: 'pending', limit: 50, offset: 200, token: 'secret', mediaUrl: 'blob:private' } as never, reviewOpen: true, token: 'secret' } as never);
  preferences.save('C:/照片 B', { filter: { sort: 'date' }, reviewOpen: false });
  const restored = new ViewPreferencesRepository(storage);
  assert.deepEqual(restored.load('C:/照片 A'), { filter: { sort: 'size', descending: true, decision: 'pending', limit: 50 }, reviewOpen: true });
  assert.equal(restored.load('C:/照片 B').filter.sort, 'date');
  const serialized = [...storage.values.values()].join('');
  assert.ok(!/secret|blob:|token|mediaUrl|offset/.test(serialized));
});

test('corrupt, oversized, future-version and invalid fields degrade to safe preferences', () => {
  for (const text of ['{broken', 'x'.repeat(9000), JSON.stringify({ version: 2, filter: { sort: 'date' } })]) {
    const storage: PreferenceStorage = { getItem: () => text, setItem() {} };
    assert.deepEqual(new ViewPreferencesRepository(storage).load('A'), { filter: { limit: 200 }, reviewOpen: false });
  }
  const storage: PreferenceStorage = { getItem: () => JSON.stringify({ version: 1, filter: { sort: 'DROP TABLE', limit: -1, descending: 'true', decision: 'erase', verdict: 'recommend', include_missing: true }, reviewOpen: 'yes' }), setItem() {} };
  assert.deepEqual(new ViewPreferencesRepository(storage).load('A'), { filter: { limit: 200, verdict: 'recommend', include_missing: true }, reviewOpen: false });
});

test('blocked or full storage retains usable memory preferences without throwing', () => {
  const storage: PreferenceStorage = { getItem() { throw new Error('blocked'); }, setItem() { throw new Error('quota'); } };
  const preferences = new ViewPreferencesRepository(storage);
  assert.deepEqual(preferences.load('A'), { filter: { limit: 200 }, reviewOpen: false });
  preferences.save('A', { filter: { sort: 'suggestion', descending: true }, reviewOpen: true });
  assert.equal(preferences.load('A').filter.sort, 'suggestion');
  const copy = preferences.load('A'); copy.filter.sort = 'date';
  assert.equal(preferences.load('A').filter.sort, 'suggestion');
});

test('invalid runtime filters are rejected before transport or persistence', () => {
  for (const filter of [{ sort: 'bad' }, { limit: 0 }, { limit: 1001 }, { offset: -1 }, { offset: 0.5 }, { descending: 'yes' }, { format: 'https://example.com' }]) {
    assert.throws(() => validatedPhotoFilter(filter, true), /Invalid photo filter/);
  }
});
