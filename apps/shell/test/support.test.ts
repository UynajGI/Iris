import { test } from 'node:test';
import assert from 'node:assert/strict';
import { errorReport, versionText, recordDiagnostic, type SupportInfo } from '../src/support.js';
import { helpTopics } from '../src/ui/help-content.js';
import { APP_VERSION } from '../src/generated/version.js';
import { readFile } from 'node:fs/promises';

test('support clipboard excludes filesystem paths, credentials and arbitrary error payloads', () => {
  const privateError =
    'connect failed: C:\\Users\\Person\\private.jpg token=secret-123 https://host/?key=private';
  const report = errorReport(privateError);
  assert.match(report, /服务连接失败/);
  for (const value of ['Person', 'private.jpg', 'secret-123', 'https://host'])
    assert.ok(!report.includes(value));
  assert.match(report, new RegExp(APP_VERSION.replaceAll('.', '\\.')));
});

test('version report includes component identity but not private data directories', () => {
  const info: SupportInfo = {
    version: APP_VERSION,
    build: 'abc123-dirty',
    platform: 'windows',
    architecture: 'x86_64',
    distribution: 'portable',
    debug: false,
    logs_available: true,
    data_directory: 'PRIVATE',
    log_directory: 'SECRET',
    daemon_version: '0.0.1',
  };
  const text = versionText(info);
  assert.match(text, /abc123-dirty/);
  assert.match(text, /后台版本：0.0.1/);
  assert.ok(!text.includes('PRIVATE') && !text.includes('SECRET'));
});

test('offline help distinguishes marking from destructive isolation and preserves user decisions', () => {
  const text = helpTopics.map((topic) => `${topic.title}\n${topic.body}`).join('\n');
  assert.match(text, /剔除标记不会删除照片/);
  assert.match(text, /隔离会移动照片/);
  assert.match(text, /模型建议不会自动改写/);
  assert.match(text, /先退出应用/);
});

test('diagnostic IPC sends a typed event only, never a session or application snapshot', async () => {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'window');
  const calls: unknown[] = [];
  Object.defineProperty(globalThis, 'window', {
    configurable: true,
    value: {
      __TAURI__: {
        core: {
          invoke: async (...args: unknown[]) => {
            calls.push(args);
          },
        },
      },
    },
  });
  try {
    recordDiagnostic('task_failed');
    await Promise.resolve();
    assert.deepEqual(calls, [['record_diagnostic_event', { event: 'task_failed' }]]);
  } finally {
    if (descriptor) Object.defineProperty(globalThis, 'window', descriptor);
    else Reflect.deleteProperty(globalThis, 'window');
  }
});

test('display version matches the package source', async () => {
  const manifest = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'));
  assert.equal(APP_VERSION, manifest.version);
});
