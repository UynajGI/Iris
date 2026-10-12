import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { GroupSelectionBar } from '../src/ui/GroupSelectionBar.js';
import { GroupCandidateActions } from '../src/ui/GroupCandidateActions.js';
import type { GroupSelectionControl } from '../src/ui/useGroupSelection.js';
import type { GroupSelection, Photo } from '../src/types.js';
import { IrisClient } from '../src/client.js';
import { Transport } from '../src/transport.js';

const photo: Photo = {
  id: 1,
  project_id: 2,
  path: 'synthetic.jpg',
  filename: 'synthetic.jpg',
  format: 'jpeg',
  width: 32,
  height: 32,
  mtime: 1,
  size_bytes: 1,
  missing: false,
  quarantined: false,
  decision: 'pending',
  rating: 0,
  color_label: 'none',
  analysis_status: 'missing',
};

const selection: GroupSelection = {
  group_id: 'group/一',
  intent: { target_keep: 1, locked_photo_ids: [1], face_choices: [], main_face: null },
  recommended_photo_ids: [1],
  pending_keep_photo_ids: [1],
  remaining_slots: 0,
  over_target: 0,
  needs_review: false,
  faces: [],
};

function control(value = selection): GroupSelectionControl {
  return {
    selection: value,
    busy: false,
    error: undefined,
    save: async () => {},
    chooseFace: async () => {},
    adopt: async () => {},
  };
}

test('group UI exposes target, explicit adoption and lock status without an automatic decision', () => {
  const html = renderToStaticMarkup(
    createElement(GroupSelectionBar, { control: control(), photos: [photo] })
  );
  assert.match(html, /本组保留数量/);
  assert.match(html, /本组主角人脸/);
  assert.match(html, /保留建议的 1 张/);
  const actions = renderToStaticMarkup(
    createElement(GroupCandidateActions, { control: control(), photo })
  );
  assert.match(actions, /aria-pressed="true"/);
  assert.match(actions, /解除必留锁定/);
  assert.match(actions, /建议保留/);
});

test('over-target and shortage are visible, and rejected photos cannot be locked', () => {
  const value = {
    ...selection,
    remaining_slots: 2,
    over_target: 1,
    intent: { ...selection.intent, locked_photo_ids: [] },
  };
  const html = renderToStaticMarkup(
    createElement(GroupSelectionBar, { control: control(value), photos: [photo] })
  );
  assert.match(html, /还缺 2 张/);
  assert.match(html, /超出目标 1 张/);
  const actions = renderToStaticMarkup(
    createElement(GroupCandidateActions, {
      control: control(value),
      photo: { ...photo, decision: 'reject' },
    })
  );
  assert.match(actions, /disabled=""/);
});

test('a matching face can be confirmed directly and an individual correction can be reset', () => {
  const choice = { photo_id: 1, face_index: 0, analysis_key: 'current' };
  const value: GroupSelection = {
    ...selection,
    intent: { ...selection.intent, main_face: { ...choice, photo_id: 2 }, face_choices: [choice] },
    faces: [{ ...choice, state: 'geometry_candidate' }],
  };
  const html = renderToStaticMarkup(
    createElement(GroupCandidateActions, { control: control(value), photo })
  );
  assert.match(html, /确认这是主角/);
  assert.match(html, /此片主角未出现/);
  assert.match(html, /恢复位置候选/);
});

test('group intent API encodes group ids, authenticates and preserves the correction revision', async () => {
  const requests: { url: string; method: string; body: unknown }[] = [];
  const client = new IrisClient(
    new Transport(
      { base_url: 'http://127.0.0.1:12345', token: 'synthetic', version: 'test' },
      async (input, init) => {
        assert.equal(new Headers(init?.headers).get('Authorization'), 'Bearer synthetic');
        requests.push({
          url: String(input),
          method: init!.method!,
          body: init?.body ? JSON.parse(String(init.body)) : null,
        });
        return Response.json(
          init?.method === 'POST' ? { id: 'batch', changed: 1, conflicts: [] } : selection
        );
      }
    )
  );
  const intent = {
    ...selection.intent,
    main_face: { photo_id: 1, face_index: 0, analysis_key: 'revision-key' },
  };
  await client.groupSelection(2, 'group/一');
  await client.setGroupIntent(2, 'group/一', intent);
  await client.acceptGroupSelection(2, 'group/一');
  assert.deepEqual(
    requests.map((request) => request.method),
    ['GET', 'PUT', 'POST']
  );
  assert.ok(requests.every((request) => request.url.includes('group%2F%E4%B8%80')));
  assert.deepEqual(requests[1]!.body, intent);
  assert.ok(requests[2]!.url.endsWith('/selection/accept'));
});
