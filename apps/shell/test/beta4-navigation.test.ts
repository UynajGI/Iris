import test from 'node:test';
import assert from 'node:assert/strict';
import { BrowseContextPublisher, browseTargets } from '../src/browse-context.js';
import { IrisClient } from '../src/client.js';
import { Transport } from '../src/transport.js';
import { boundedCompareView, fitCompareView, zoomCompareView } from '../src/compare-view.js';
import { validatedPhotoFilter, ViewPreferencesRepository } from '../src/view-preferences.js';

test('browse hints coalesce rapid navigation, clear on leave and reject further publication', async () => {
  const requests: { revision: number; focused_photo_id: number | null }[] = [];
  const client = new IrisClient(
    new Transport(
      { base_url: 'http://127.0.0.1:45678', token: 'fixture', version: '0.1.0' },
      async (_input, init) => {
        requests.push(JSON.parse(String(init?.body)));
        return Response.json({});
      }
    )
  );
  const publisher = new BrowseContextPublisher(client, 1);
  try {
    publisher.update(browseTargets(1, [], [1, 2, 3]));
    publisher.update(browseTargets(2, [3], [1, 2, 3]));
    await new Promise((resolve) => setTimeout(resolve, 100));
    assert.equal(requests.length, 1);
    assert.equal(requests[0]!.focused_photo_id, 2);
    publisher.dispose();
    assert.equal(requests.length, 2);
    assert.equal(requests[1]!.focused_photo_id, null);
    assert.ok(requests[1]!.revision > requests[0]!.revision);
    publisher.update(browseTargets(3, [], [3]));
    await new Promise((resolve) => setTimeout(resolve, 100));
    assert.equal(requests.length, 2);
  } finally {
    publisher.dispose();
  }
});

test('browse neighbors are bounded, omit higher priority targets and follow current ordering', () => {
  const ids = Array.from({ length: 100 }, (_, index) => index + 1);
  const targets = browseTargets(50, [49, 51], ids);
  assert.equal(targets.nearby_photo_ids.length, 22);
  assert.equal(targets.nearby_photo_ids[0], 38);
  assert.equal(targets.nearby_photo_ids.at(-1), 62);
  assert.ok(!targets.nearby_photo_ids.some((id) => [49, 50, 51].includes(id)));
});

test('linked view bounds keep the image reachable through extreme zoom and pan', () => {
  assert.deepEqual(boundedCompareView({ scale: 1, x: 30, y: -20 }), fitCompareView);
  assert.deepEqual(boundedCompareView({ scale: 100, x: 40, y: -50 }), {
    scale: 6,
    x: 2.5,
    y: -2.5,
  });
  assert.deepEqual(boundedCompareView({ scale: NaN, x: Infinity, y: NaN }), fitCompareView);
  assert.deepEqual(zoomCompareView({ scale: 2, x: 0.2, y: -0.1 }, 0.5), fitCompareView);
});

test('scene and exact-hit scopes survive live filters, but never persist across reopen', () => {
  const filter = validatedPhotoFilter(
    { scene_id: 'scene-fixture', photo_id: 12, rating: 3, offset: 200 },
    true
  );
  assert.equal(filter.scene_id, 'scene-fixture');
  assert.equal(filter.photo_id, 12);
  const preferences = new ViewPreferencesRepository();
  preferences.save('synthetic-fixture', { filter, reviewOpen: false });
  assert.deepEqual(preferences.load('synthetic-fixture').filter, { limit: 200, rating: 3 });
  assert.throws(() => validatedPhotoFilter({ scene_id: 'x'.repeat(129) }, true));
  assert.throws(() => validatedPhotoFilter({ photo_id: 0 }, true));
});
