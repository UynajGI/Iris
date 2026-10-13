import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  applyFilterPreset,
  FilterPresetsRepository,
  FILTER_PRESET_KEY,
  MAX_FILTER_PRESETS,
} from '../src/filter-presets.js';
import type { PreferenceStorage } from '../src/view-preferences.js';

class MemoryStorage implements PreferenceStorage {
  text: string | null = null;
  getItem() {
    return this.text;
  }
  setItem(key: string, value: string) {
    assert.equal(key, FILTER_PRESET_KEY);
    this.text = value;
  }
}

test('named combinations survive repository recreation without project, photo or page scope', () => {
  const backing = new MemoryStorage();
  new FilterPresetsRepository(backing).add('  保留五星  ', {
    decision: 'keep',
    rating: 5,
    sort: 'name',
    descending: true,
    scene_id: 'private-scene',
    photo_id: 12,
    offset: 400,
    limit: 50,
    include_missing: true,
    token: 'secret',
  } as never);
  const restored = new FilterPresetsRepository(backing).load();
  assert.deepEqual(restored, [
    {
      name: '保留五星',
      filter: {
        decision: 'keep',
        rating: 5,
        sort: 'name',
        descending: true,
      },
    },
  ]);
  assert.ok(!/private|secret|photo_id|offset|limit|include_missing/.test(backing.text!));
  const applied = applyFilterPreset(
    {
      decision: 'reject',
      color_label: 'red',
      scene_id: 'other',
      photo_id: 99,
      offset: 200,
      limit: 100,
    },
    restored[0]!.filter
  );
  assert.deepEqual(applied, {
    decision: 'keep',
    rating: 5,
    sort: 'name',
    descending: true,
    offset: 0,
    limit: 100,
  });
  // Callers cannot mutate persisted combinations through a returned object.
  restored[0]!.filter.rating = 1;
  assert.equal(new FilterPresetsRepository(backing).load()[0]!.filter.rating, 5);
});

test('duplicate names are not overwritten and deletion persists without changing current filters', () => {
  const backing = new MemoryStorage();
  const repository = new FilterPresetsRepository(backing);
  repository.add('待定 RAW', { decision: 'flag', format: 'raw' });
  assert.throws(() => repository.add(' 待定 RAW ', { decision: 'keep' }), /名称已存在/);
  assert.equal(repository.load()[0]!.filter.decision, 'flag');
  assert.deepEqual(repository.remove('待定 RAW'), []);
  assert.deepEqual(new FilterPresetsRepository(backing).load(), []);
});

test('names, count and untrusted storage are bounded', () => {
  const backing = new MemoryStorage();
  const repository = new FilterPresetsRepository(backing);
  for (const name of ['', '  ', '长'.repeat(41)])
    assert.throws(() => repository.add(name, {}), /1–40/);
  for (let i = 0; i < MAX_FILTER_PRESETS; i++) repository.add(`组合 ${i}`, {});
  assert.throws(() => repository.add('多一个', {}), /最多保存/);
  for (const invalid of ['{', 'null', 'x'.repeat(16385), '{"version":2,"presets":[]}']) {
    backing.text = invalid;
    assert.deepEqual(repository.load(), []);
  }
  backing.text = JSON.stringify({
    version: 1,
    presets: [
      null,
      { name: '' },
      { name: 'A', filter: { decision: 'erase', rating: 90, format: 'raw', scene_id: 'private' } },
      { name: ' A ', filter: { decision: 'keep' } },
    ],
  });
  assert.deepEqual(repository.load(), [{ name: 'A', filter: { format: 'raw' } }]);
});

test('storage write failure is reported rather than claiming a save or deletion succeeded', () => {
  const backing = new MemoryStorage();
  new FilterPresetsRepository(backing).add('原组合', { rating: 3 });
  const broken = new FilterPresetsRepository({
    getItem: () => backing.text,
    setItem: () => {
      throw new Error('quota exceeded');
    },
  });
  assert.throws(() => broken.add('新组合', {}), /无法保存/);
  assert.throws(() => broken.remove('原组合'), /无法保存/);
  assert.equal(broken.load()[0]!.name, '原组合');
});
