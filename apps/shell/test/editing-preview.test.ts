import { test } from 'node:test';
import assert from 'node:assert/strict';
import { setImmediate } from 'node:timers/promises';
import { EditingPreviewQueue } from '../src/editing-preview-queue.js';

function deferred() {
  let resolve!: (result: number) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<number>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
function harness() {
  const calls: { input: number; signal: AbortSignal; job: ReturnType<typeof deferred> }[] = [];
  const results: number[] = [],
    errors: unknown[] = [],
    cancellations: number[] = [],
    busy: boolean[] = [];
  const queue = new EditingPreviewQueue<number, number>({
    render(input, signal) {
      const job = deferred();
      calls.push({ input, signal, job });
      return job.promise;
    },
    result(value) {
      results.push(value);
    },
    error(reason) {
      errors.push(reason);
    },
    busy(value) {
      busy.push(value);
    },
    cancel(input) {
      cancellations.push(input);
    },
  });
  return { queue, calls, results, errors, cancellations, busy };
}

test('continuous input displays completed feedback before consuming the latest pending input', async () => {
  const h = harness();
  h.queue.push(1);
  h.queue.push(2);
  h.queue.push(3);
  assert.deepEqual(
    h.calls.map((c) => c.input),
    [1]
  );
  assert.equal(h.calls[0]!.signal.aborted, false);
  h.calls[0]!.job.resolve(1);
  await setImmediate();
  assert.deepEqual(h.results, [1]);
  assert.deepEqual(
    h.calls.map((c) => c.input),
    [1, 3]
  );
  h.queue.push(4);
  h.queue.push(5);
  h.calls[1]!.job.resolve(3);
  await setImmediate();
  assert.deepEqual(h.results, [1, 3]);
  assert.deepEqual(
    h.calls.map((c) => c.input),
    [1, 3, 5]
  );
  h.calls[2]!.job.resolve(5);
  await setImmediate();
  assert.deepEqual(h.results, [1, 3, 5]);
  assert.deepEqual(h.busy, [true, false]);
});
test('closing an editor aborts work and never displays a late result', async () => {
  const h = harness();
  h.queue.push(1);
  h.queue.push(2);
  h.queue.dispose();
  assert.equal(h.calls[0]!.signal.aborted, true);
  assert.deepEqual(h.cancellations, [1]);
  h.calls[0]!.job.resolve(1);
  await setImmediate();
  h.queue.push(3);
  assert.deepEqual(h.results, []);
  assert.equal(h.calls.length, 1);
});
test('a superseded failure does not hide the next successful preview', async () => {
  const h = harness();
  h.queue.push(1);
  h.queue.push(2);
  h.calls[0]!.job.reject(new Error('old failure'));
  await setImmediate();
  assert.deepEqual(h.errors, []);
  h.calls[1]!.job.resolve(2);
  await setImmediate();
  assert.deepEqual(h.results, [2]);
  h.queue.push(3);
  h.calls[2]!.job.reject(new Error('current failure'));
  await setImmediate();
  assert.equal(h.errors.length, 1);
  assert.equal(h.busy.at(-1), false);
});
test('new interaction can interrupt idle refinement without publishing the cancelled image', async () => {
  const h = harness();
  h.queue.push(1600);
  h.queue.push(768, true);
  assert.equal(h.calls[0]!.signal.aborted, true);
  assert.deepEqual(h.cancellations, [1600]);
  h.calls[0]!.job.resolve(1600);
  await setImmediate();
  assert.deepEqual(h.results, []);
  assert.equal(h.calls[1]!.input, 768);
  h.calls[1]!.job.resolve(768);
  await setImmediate();
  assert.deepEqual(h.results, [768]);
});
