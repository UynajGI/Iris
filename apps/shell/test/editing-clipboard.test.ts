import { test } from 'node:test';
import assert from 'node:assert/strict';
import { AdjustmentClipboard, adjustmentClipboardShortcut } from '../src/editing-clipboard.js';
import {
  defaultAdjustments,
  defaultDetail,
  defaultGeometry,
  parameters,
  updatePanel,
} from '../src/editing-document.js';
import type { EditDocument } from '../src/editing-client.js';

function document(): EditDocument {
  return {
    schema_version: 1,
    source: {
      photo_id: '1',
      fingerprint: 'a'.repeat(64),
      width: 32,
      height: 24,
      reference_kind: 'display',
    },
    nodes: [
      {
        id: 'source',
        name: '原片',
        version: 1,
        enabled: true,
        inputs: [],
        operation: { type: 'source' },
      },
    ],
    output_node: 'source',
    output: {
      space: 'srgb',
      tone_map: { kind: 'none' },
      gamut_compression: true,
      background: [1, 1, 1],
    },
    panel_bindings: [],
    node_positions: {},
  };
}

test('clipboard captures unsaved values once and repeated pastes do not share mutable controls', () => {
  const clipboard = new AdjustmentClipboard();
  assert.equal(clipboard.available, false);
  assert.throws(() => clipboard.paste(document()), /请先复制/);
  const source = updatePanel(document(), {
    type: 'adjustments',
    parameters: { ...defaultAdjustments(), exposure: 1.25 },
  });
  clipboard.copy(source);
  assert.equal(clipboard.available, true);
  parameters(source, 'adjustments').exposure = 2;
  const first = clipboard.paste(document());
  assert.equal(parameters(first, 'adjustments').exposure, 1.25);
  parameters(first, 'adjustments').exposure = -1;
  assert.equal(parameters(clipboard.paste(document()), 'adjustments').exposure, 1.25);
  clipboard.copy(source);
  assert.equal(parameters(clipboard.paste(document()), 'adjustments').exposure, 2);
});

test('paste uses preset merge and keeps target source, local nodes, geometry, resources and output', () => {
  const clipboard = new AdjustmentClipboard();
  let source = updatePanel(document(), {
    type: 'adjustments',
    parameters: { ...defaultAdjustments(), exposure: 0.75, saturation: 1.2 },
  });
  source = updatePanel(source, {
    type: 'detail',
    parameters: { ...defaultDetail(), clarity: 0.25 },
  });
  clipboard.copy(source);
  let target = updatePanel(document(), {
    type: 'geometry',
    parameters: { ...defaultGeometry(), rotation_degrees: 5 },
  });
  target.source = {
    ...target.source,
    photo_id: '2',
    fingerprint: 'b'.repeat(64),
    raw_white_balance: [1.8, 1, 1.3],
  };
  target.output.space = 'display_p3';
  target.nodes.push({
    id: 'local',
    name: '局部',
    version: 1,
    enabled: true,
    inputs: [target.output_node],
    operation: { type: 'adjustments', parameters: { ...defaultAdjustments(), exposure: -0.5 } },
  });
  target.output_node = 'local';
  const original = structuredClone(target);
  const pasted = clipboard.paste(target);
  assert.deepEqual(target, original);
  assert.deepEqual(pasted.source, original.source);
  assert.deepEqual(pasted.output, original.output);
  assert.deepEqual(parameters(pasted, 'geometry'), parameters(original, 'geometry'));
  assert.deepEqual(
    pasted.nodes.find((node) => node.id === 'local')?.operation,
    original.nodes.find((node) => node.id === 'local')?.operation
  );
  assert.equal(parameters(pasted, 'adjustments').exposure, 0.75);
  assert.equal(parameters(pasted, 'detail').clarity, 0.25);
});

function key(patch: Partial<KeyboardEvent> = {}) {
  return { key: 'C', ctrlKey: true, shiftKey: false, ...patch } as KeyboardEvent;
}
test('grading clipboard shortcuts accept platform modifiers without stealing text or dialog input', () => {
  assert.equal(adjustmentClipboardShortcut(key()), 'copy');
  assert.equal(
    adjustmentClipboardShortcut(key({ ctrlKey: false, metaKey: true, key: 'v' })),
    'paste'
  );
  for (const patch of [
    { shiftKey: true },
    { ctrlKey: false },
    { altKey: true },
    { repeat: true },
    { isComposing: true },
    { defaultPrevented: true },
    { key: 'a' },
    { target: { tagName: 'INPUT' } },
    { target: { tagName: 'TEXTAREA' } },
    { target: { tagName: 'SELECT' } },
    { target: { isContentEditable: true } },
    { target: { closest: () => ({}) } },
  ])
    assert.equal(adjustmentClipboardShortcut(key(patch as Partial<KeyboardEvent>)), null);
});
