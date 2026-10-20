import test from 'node:test';
import assert from 'node:assert/strict';
import { EditingSaveQueue } from '../src/editing-save-queue.js';

test('navigation joins autosave and waits for edits made during the first write', async () => {
  const queue = new EditingSaveQueue();
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  let draft = 1;
  let persisted = 0;
  const writes: number[] = [];
  const step = async () => {
    const submitted = draft;
    writes.push(submitted);
    if (writes.length === 1) await gate;
    persisted = submitted;
    return persisted === draft ? ('clean' as const) : ('pending' as const);
  };
  const background = queue.flush(step);
  draft = 2;
  const navigation = queue.flush(step);
  assert.equal(background, navigation);
  assert.deepEqual(writes, [1]);
  release();
  assert.equal(await navigation, true);
  assert.deepEqual(writes, [1, 2]);
  assert.equal(persisted, 2);
});

test('a failed write stops the drain and can be retried without reporting success', async () => {
  const queue = new EditingSaveQueue();
  let calls = 0;
  assert.equal(
    await queue.flush(async () => {
      calls++;
      return 'failed';
    }),
    false
  );
  assert.equal(calls, 1);
  assert.equal(await queue.flush(async () => 'clean'), true);
});

test('an unexpected rejection releases the queue for recovery', async () => {
  const queue = new EditingSaveQueue();
  await assert.rejects(
    queue.flush(async () => {
      throw new Error('offline');
    }),
    /offline/
  );
  assert.equal(await queue.flush(async () => 'clean'), true);
});
