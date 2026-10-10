import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { root } from '../backend.mjs';
import { verifySchema } from '../api.mjs';

function make(args) {
  return spawnSync(process.env.MAKE || 'make', args, { cwd: root, encoding: 'utf8', env: { ...process.env, MAKEFLAGS: '', MFLAGS: '' } });
}
test('release builds include core, RAW and desktop with locked dependencies', () => {
  const result = make(['-n', 'build', 'build-desktop', 'PROFILE=release']);
  assert.equal(result.status, 0, result.stderr);
  const cargo = result.stdout.split('\n').filter(line => line.startsWith('cargo build'));
  assert.equal(cargo.length, 3);
  assert.ok(cargo.every(line => line.includes('--locked') && line.includes('--release')));
  assert.match(result.stdout, /components\/raw-decoder\/Cargo.toml/);
});
test('setup prepares environment before locked dependencies and hooks, without optional downloads', () => {
  const result = make(['-n', 'setup']);
  assert.equal(result.status, 0, result.stderr);
  assert.ok(result.stdout.indexOf('development.mjs env') < result.stdout.indexOf('npm ci'));
  assert.ok(result.stdout.indexOf('npm --prefix apps/shell ci') < result.stdout.indexOf('install-hooks.mjs'));
  assert.doesNotMatch(result.stdout, /setup-models|codegraph|setup-dinov3/);
});
test('invalid profile and missing output fail before provisioning', () => {
  assert.notEqual(make(['build', 'PROFILE=invalid']).status, 0);
  const result = make(['source', 'OUTPUT=']);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Provide OUTPUT/);
  assert.doesNotMatch(result.stdout, /package-source.py/);
});
test('output paths containing spaces remain quoted', () => {
  const result = make(['-n', 'source', 'OUTPUT=dist/source export/archive.zip']);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /--output "dist\/source export\/archive.zip"/);
});
test('test group stops at first failure even with -j', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'iris-make-'));
  try {
    const stub = resolve(dir, 'fail.mjs');
    writeFileSync(stub, 'console.log("INTENTIONAL_FAILURE"); process.exit(23);');
    const result = make(['-j4', 'test', `CARGO=node "${stub.replaceAll('\\', '/')}"`]);
    assert.notEqual(result.status, 0);
    assert.match(result.stdout, /INTENTIONAL_FAILURE/);
    assert.doesNotMatch(result.stdout, /development.test.mjs|unittest|npm --prefix/);
  } finally { rmSync(dir, { recursive: true }); }
});
test('API comparison rejects drift and does not depend on JSON key order', () => {
  const schema = { paths: { '/api/v1/bootstrap': {} }, info: { version: 'new' } };
  assert.throws(() => verifySchema(JSON.stringify(schema), JSON.stringify({ ...schema, info: { version: 'old' } }), true), /stale/);
  assert.deepEqual(verifySchema(JSON.stringify(schema), JSON.stringify({ info: schema.info, paths: schema.paths }), true), schema);
  assert.throws(() => verifySchema('{"paths":{}}', '', false), /Unexpected/);
});
test('backend argument boundary is preserved for paths with spaces', () => {
  const dir = mkdtempSync(resolve(tmpdir(), 'iris-backend-'));
  try {
    const stub = resolve(dir, 'arguments.mjs');
    writeFileSync(stub, 'console.log(JSON.stringify(process.argv.slice(2)));');
    const result = spawnSync(process.execPath, ['tools/backend.mjs', stub, '--output', 'folder with spaces/archive.zip'], { cwd: root, encoding: 'utf8', env: { ...process.env, IRIS_BACKEND: process.execPath } });
    assert.equal(result.status, 0, result.stderr);
    assert.deepEqual(JSON.parse(result.stdout), ['--output', 'folder with spaces/archive.zip']);
  } finally { rmSync(dir, { recursive: true }); }
});
test('Development instructions expose only Make commands', () => {
  for (const file of ['README.md', 'zh-CN/README.md']) {
    const text = readFileSync(resolve(root, file), 'utf8');
    const section = text.split(/## 🔧 (?:Development|开发)/)[1].split(/\n## /)[0];
    assert.doesNotMatch(section, /python|dev\.py/i);
    assert.match(section, /make setup/);
  }
});
