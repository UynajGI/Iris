import { test } from 'node:test';
import assert from 'node:assert/strict';
import { MediaCache, previewNeighbors } from '../src/media-cache.js';
import type { Photo } from '../src/types.js';

test('shared consumers deduplicate and a releasing consumer cannot abort another', async () => {
  let calls = 0;
  let signal: AbortSignal | undefined;
  let finish!: (value: Blob) => void;
  const cache = new MediaCache({
    media: (_id, _kind, s) => {
      calls++;
      signal = s;
      return new Promise((resolve) => {
        finish = resolve;
      });
    },
  });
  const a = cache.acquire(1, 'preview', 'r1'),
    b = cache.acquire(1, 'preview', 'r1');
  a.release();
  assert.equal(signal?.aborted, false);
  finish(new Blob(['preview']));
  await b.promise;
  b.release();
  const c = cache.acquire(1, 'preview', 'r1');
  await c.promise;
  c.release();
  assert.equal(calls, 1);
});

test('abandoned pending requests cancel; a new revision fetches fresh bytes', async () => {
  const signals: AbortSignal[] = [];
  const cache = new MediaCache({
    media: (_id, _kind, signal) => {
      signals.push(signal!);
      return new Promise<Blob>(() => {});
    },
  });
  const a = cache.acquire(1, 'preview', 'r1');
  a.release();
  a.release();
  assert.equal(signals[0]!.aborted, true);
  const b = cache.acquire(1, 'preview', 'r2');
  assert.equal(signals.length, 2);
  b.release();
});

test('retained bytes and entries are bounded; originals are never retained after release', async () => {
  let calls = 0;
  const cache = new MediaCache(
    {
      media: async () => {
        calls++;
        return new Blob(['123456']);
      },
    },
    2,
    10
  );
  const get = async (id: number, kind: 'preview' | 'original' = 'preview') => {
    const lease = cache.acquire(id, kind);
    await lease.promise;
    lease.release();
  };
  await get(1);
  await get(2);
  await get(1);
  assert.equal(calls, 3, 'byte budget evicts oldest unused preview');
  await get(3, 'original');
  await get(3, 'original');
  assert.equal(calls, 5, 'originals only shared by live consumers');
});

test('errors can be retried and an old rejection cannot evict its replacement', async () => {
  let calls = 0;
  let rejectOld!: (error: Error) => void;
  const cache = new MediaCache({
    media: async () => {
      calls++;
      if (calls === 1)
        return new Promise<Blob>((_resolve, reject) => {
          rejectOld = reject;
        });
      return new Blob(['fresh']);
    },
  });
  const a = cache.acquire(1, 'preview');
  a.release();
  const b = cache.acquire(1, 'preview');
  rejectOld(new Error('late abort'));
  await assert.rejects(a.promise);
  await b.promise;
  b.release();
  const c = cache.acquire(1, 'preview');
  await c.promise;
  c.release();
  assert.equal(calls, 2);
});

test('decode failure invalidates only its own lease and permits fresh bytes', async () => {
  let calls = 0;
  const cache = new MediaCache({ media: async () => new Blob([String(++calls)]) });
  const a = cache.acquire(1, 'preview');
  await a.promise;
  a.invalidate();
  const b = cache.acquire(1, 'preview');
  assert.equal(await (await b.promise).text(), '2');
  a.invalidate();
  a.release();
  b.release();
  const c = cache.acquire(1, 'preview');
  await c.promise;
  c.release();
  assert.equal(calls, 2, 'late decode failure cannot evict a newer request');
});

test('prefetch selects only two immediate available neighbors', () => {
  const photos = [1, 2, 3, 4, 5].map(
    (id) => ({ id, missing: id === 2, quarantined: false }) as Photo
  );
  assert.deepEqual(
    previewNeighbors(photos, 3).map((p) => p.id),
    [4]
  );
  assert.deepEqual(
    previewNeighbors(photos, 4).map((p) => p.id),
    [5, 3]
  );
  assert.deepEqual(previewNeighbors(photos, 99), []);
});

test('entry budget independently evicts the least recently used idle blob', async () => {
  let calls = 0;
  const cache = new MediaCache({ media: async () => new Blob([String(++calls)]) }, 2, 1_000_000);
  for (const id of [1, 2, 1, 3, 1, 2]) {
    const lease = cache.acquire(id, 'preview');
    await lease.promise;
    lease.release();
  }
  assert.equal(calls, 4, 'recently used 1 survives, idle 2 is fetched again');
});

test('expired idle blobs revalidate without aborting active consumers', async () => {
  let time = 0,
    calls = 0;
  const signals: AbortSignal[] = [];
  const cache = new MediaCache(
    {
      media: async (_id, _kind, signal) => {
        signals.push(signal!);
        return new Blob([String(++calls)]);
      },
    },
    24,
    1_000_000,
    30_000,
    () => time
  );
  const first = cache.acquire(1, 'preview');
  await first.promise;
  time = 31_000;
  const shared = cache.acquire(1, 'preview');
  assert.equal(await (await shared.promise).text(), '1');
  first.release();
  shared.release();
  assert.equal(signals[0]!.aborted, false);
  const fresh = cache.acquire(1, 'preview');
  assert.equal(await (await fresh.promise).text(), '2');
  assert.equal(signals[0]!.aborted, true);
  fresh.release();
});
