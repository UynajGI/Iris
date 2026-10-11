import { test } from 'node:test';
import assert from 'node:assert/strict';
import { workspaceShortcut } from '../src/ui/workspace-shortcuts.js';
import { CommandSystem } from '../src/commands.js';
import type { IrisStore } from '../src/store.js';

const key = (value: string, extra: Record<string, unknown> = {}) =>
  ({ key: value, target: null, ...extra }) as unknown as KeyboardEvent;

test('workspace shortcuts preserve Tab, Shift+Tab and native editing', () => {
  for (const value of ['Tab', 'Enter', ' ', 'Escape']) {
    assert.equal(workspaceShortcut(key(value)), null);
    assert.equal(workspaceShortcut(key(value, { shiftKey: true })), null);
  }
  for (const target of [
    { tagName: 'INPUT' },
    { tagName: 'TEXTAREA' },
    { tagName: 'SELECT' },
    { isContentEditable: true },
    { closest: () => ({}) },
  ]) {
    for (const value of ['b', 'g', '5', '?', '+'])
      assert.equal(workspaceShortcut(key(value, { target })), null);
  }
  for (const modifier of [
    'repeat',
    'isComposing',
    'defaultPrevented',
    'ctrlKey',
    'metaKey',
    'altKey',
  ]) {
    assert.equal(workspaceShortcut(key('5', { [modifier]: true })), null);
  }
});

test('workspace shortcuts distinguish rating, zoom, panels and undo', () => {
  for (let rating = 0; rating <= 5; rating++)
    assert.equal(workspaceShortcut(key(String(rating))), rating);
  assert.equal(workspaceShortcut(key('b')), 'panels');
  assert.equal(workspaceShortcut(key('g')), 'grid');
  assert.equal(workspaceShortcut(key('e')), 'loupe');
  assert.equal(workspaceShortcut(key('i')), 'faces');
  assert.equal(workspaceShortcut(key('f')), 'fullscreen');
  assert.equal(workspaceShortcut(key('? ', { shiftKey: true })), null);
  assert.equal(workspaceShortcut(key('?', { shiftKey: true })), 'help');
  assert.equal(workspaceShortcut(key('+', { shiftKey: true })), 'zoomIn');
  assert.equal(workspaceShortcut(key('=')), 'zoomIn');
  assert.equal(workspaceShortcut(key('-')), 'zoomOut');
  assert.equal(workspaceShortcut(key('Z', { shiftKey: true })), 'zoomToggle');
  assert.equal(workspaceShortcut(key('z', { shiftKey: true })), 'zoomToggle');
  assert.equal(workspaceShortcut(key('z')), null);
  assert.equal(workspaceShortcut(key('6')), null);
});

test('Shift+Z reaches zoom after CommandSystem without executing undo', async () => {
  let undos = 0;
  const commands = new CommandSystem({
    getSnapshot: () => ({ groupSession: null, reviewOpen: true }),
    undo: async () => {
      undos++;
    },
  } as unknown as IrisStore);
  for (const value of ['Z', 'z']) {
    const event = key(value, { shiftKey: true, preventDefault() {} });
    assert.equal(commands.handle(event), false);
    assert.equal(workspaceShortcut(event), 'zoomToggle');
  }
  assert.equal(undos, 0);
  assert.equal(commands.handle(key('z', { preventDefault() {} })), true);
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(undos, 1);
});
