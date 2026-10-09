import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createAppTitleSynchronizer, type NativeTitleBridge } from '../src/native-title.js';
import type { Locale } from '../src/i18n.js';

test('native and document titles follow Chinese variants and the English fallback', async () => {
  const document = { title: '' };
  const calls: unknown[] = [];
  const bridge: NativeTitleBridge = { async invoke<T>(command: string, args: unknown) {
    calls.push({ command, args });
    return undefined as T;
  } };
  const synchronize = createAppTitleSynchronizer(bridge, document);
  for (const [language, expected, locale] of [
    ['zh-CN', '伊人', 'zh-CN'], ['zh-TW', '伊人', 'zh-CN'], ['ZH-Hant', '伊人', 'zh-CN'],
    ['en-US', 'IrisVision', 'en'], ['fr-FR', 'IrisVision', 'en'],
  ]) {
    await synchronize(language!);
    assert.equal(document.title, expected);
    assert.deepEqual(calls.at(-1), { command: 'set_app_locale', args: { locale } });
  }
});

test('overlapping language changes cannot leave native and document titles on an older language', async () => {
  const document = { title: 'IrisVision' };
  const locales: string[] = [];
  let release!: () => void;
  const held = new Promise<void>(resolve => { release = resolve; });
  let started!: () => void;
  const firstStarted = new Promise<void>(resolve => { started = resolve; });
  const bridge: NativeTitleBridge = { async invoke<T>(_command: string, args: { locale: Locale }) {
    locales.push(args.locale);
    if (locales.length === 1) { started(); await held; }
    return undefined as T;
  } };
  const synchronize = createAppTitleSynchronizer(bridge, document);
  const first = synchronize('zh-CN'); await firstStarted;
  const second = synchronize('en-US');
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(locales, ['zh-CN']);
  release(); await Promise.all([first, second]);
  assert.deepEqual(locales, ['zh-CN', 'en']);
  assert.equal(document.title, 'IrisVision');
});

test('native title errors propagate and do not block the next language change', async () => {
  const document = { title: 'IrisVision' };
  let fail = true;
  const bridge: NativeTitleBridge = { async invoke<T>() {
    if (fail) throw new Error('native title unavailable');
    return undefined as T;
  } };
  const synchronize = createAppTitleSynchronizer(bridge, document);
  await assert.rejects(synchronize('zh-CN'), /native title unavailable/);
  assert.equal(document.title, 'IrisVision');
  fail = false;
  await synchronize('zh-CN');
  assert.equal(document.title, '伊人');
});
