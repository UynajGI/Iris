import { test } from 'node:test';
import assert from 'node:assert/strict';
import { exportLocation, openExportLocation } from '../src/export-location.js';

test('copy, CSV and XMP retain the correct completed export destination', () => {
  assert.deepEqual(exportLocation('copy', 'D:\\已选照片', 'C:\\原片'), {
    path: 'D:\\已选照片',
    file: false,
  });
  assert.deepEqual(exportLocation('csv', '/exports/选片.csv', '/photos'), {
    path: '/exports/选片.csv',
    file: true,
  });
  assert.deepEqual(exportLocation('xmp', 'old dialog destination', '/photos'), {
    path: '/photos',
    file: false,
  });
});

test('opening an export uses the native directory command and propagates errors', async () => {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const calls: unknown[] = [];
  const fixture = {
    __TAURI__: {
      core: {
        invoke: async (...args: unknown[]) => {
          calls.push(args);
        },
      },
    },
  };
  Object.defineProperty(globalThis, 'window', { configurable: true, value: fixture });
  try {
    await openExportLocation({ path: '/exports/选片.csv', file: true });
    assert.deepEqual(calls, [['open_export_folder', { path: '/exports/选片.csv', file: true }]]);
    fixture.__TAURI__.core.invoke = async () => {
      throw new Error('目录已不存在');
    };
    await assert.rejects(openExportLocation({ path: '/gone', file: false }), /目录已不存在/);
    Object.defineProperty(globalThis, 'window', { configurable: true, value: {} });
    await assert.rejects(openExportLocation({ path: '/exports', file: false }), /桌面应用/);
  } finally {
    if (descriptor) Object.defineProperty(globalThis, 'window', descriptor);
    else Reflect.deleteProperty(globalThis, 'window');
  }
});
