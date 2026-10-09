import test from 'node:test';
import assert from 'node:assert/strict';
import { openProjectFolder, selectProjectFolder, type NativeDialogBridge } from '../src/native-dialog.js';

function bridge(value: unknown): NativeDialogBridge {
  return { async invoke<T>(command: string): Promise<T> {
    assert.equal(command, 'select_project_folder');
    return value as T;
  } };
}

test('native folder cancellation leaves the current project untouched', async () => {
  let opens = 0;
  assert.equal(await openProjectFolder(bridge(null), { async openProject() { opens++; } }), false);
  assert.equal(opens, 0);
});

test('native folder selection preserves Unicode and spaces, and propagates open failure', async () => {
  const root = 'C:\\照片集\\婚礼 2026';
  let opened = '';
  assert.equal(await openProjectFolder(bridge(root), { async openProject(path) { opened = path; } }), true);
  assert.equal(opened, root);
  await assert.rejects(openProjectFolder(bridge(root), { async openProject() { throw new Error('unavailable'); } }), /unavailable/);
  for (const invalid of ['', undefined, [], 42]) await assert.rejects(selectProjectFolder(bridge(invalid)), /Invalid native/);
});
