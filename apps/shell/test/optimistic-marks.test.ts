import { test } from 'node:test';
import assert from 'node:assert/strict';
import { OptimisticMarks } from '../src/optimistic-marks.js';
import type { Photo } from '../src/types.js';

const photo = { id: 1, rating: 1, color_label: 'none', decision: 'pending' } as Photo;
test('pending marks overlay stale refreshes and failed predecessors cannot erase later edits', () => {
  const marks = new OptimisticMarks();
  const first = marks.add({ photo_ids: [1], rating: 3 }, [photo]);
  const second = marks.add({ photo_ids: [1], rating: 5, color_label: 'red' }, [marks.apply(photo)]);
  assert.equal(marks.apply(photo).rating, 5);
  let current = marks.settle(first, true)(marks.apply(photo));
  assert.equal(current.rating, 5);
  current = marks.settle(second, true)(current);
  assert.equal(current.rating, 1);
  assert.equal(current.color_label, 'none');
  assert.equal(marks.pending, false);
});
test('failed newer mark rolls back to successful predecessor and preserves other fields', () => {
  const marks = new OptimisticMarks();
  const first = marks.add({ photo_ids: [1], rating: 3 }, [photo]);
  const second = marks.add({ photo_ids: [1], rating: 5 }, [marks.apply(photo)]);
  let current = marks.settle(first, false)(marks.apply(photo));
  current = marks.settle(second, true)({ ...current, color_label: 'blue' });
  assert.equal(current.rating, 3);
  assert.equal(current.color_label, 'blue');
});
test('null fields are omitted and retiring a project clears pending overlays', () => {
  const marks = new OptimisticMarks();
  marks.add({ photo_ids: [1], rating: null, color_label: 'green' }, [photo]);
  assert.equal(marks.apply(photo).rating, 1);
  marks.clear();
  assert.deepEqual(marks.apply(photo), photo);
});
