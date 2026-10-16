import test from 'node:test';
import assert from 'node:assert/strict';
import { PreviewFrames } from '../src/preview-frames.js';

test('preview holds the old frame while decoding; late frames and disposal never publish', async () => {
  const pending: (() => void)[] = [];
  const frames = new PreviewFrames(() => new Promise<void>((resolve) => pending.push(resolve)));
  const shown: string[] = [];
  const accept = (url: string) => {
    shown.push(url);
    return true;
  };
  const fail = () => assert.fail('unexpected decode error');
  const first = frames.show(new Blob(['one']), accept, fail);
  assert.equal(shown.length, 0);
  pending[0]!();
  await first;
  assert.equal(shown.length, 1);
  const slow = frames.show(new Blob(['slow']), accept, fail);
  const fast = frames.show(new Blob(['fast']), accept, fail);
  assert.equal(shown.length, 1);
  pending[2]!();
  await fast;
  pending[1]!();
  await slow;
  assert.equal(shown.length, 2);
  const disposed = frames.show(new Blob(['disposed']), accept, fail);
  frames.dispose();
  pending[3]!();
  await disposed;
  assert.equal(shown.length, 2);
});
test('failed or rejected decodes preserve the previous frame', async () => {
  let reject = false;
  const frames = new PreviewFrames(async () => {
    if (reject) throw new Error('decode failed');
  });
  let count = 0,
    errors = 0;
  const accept = () => {
    ++count;
    return true;
  };
  await frames.show(new Blob(), accept, () => ++errors);
  await frames.show(
    new Blob(),
    () => false,
    () => ++errors
  );
  reject = true;
  await frames.show(new Blob(), accept, () => ++errors);
  assert.equal(count, 1);
  assert.equal(errors, 1);
  frames.dispose();
});
