import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

test('portable copy excludes optional and unlisted model weights before copying', () => {
  const result = spawnSync('pwsh', ['-NoProfile', '-File', fileURLToPath(new URL('./package-models.test.ps1', import.meta.url))], { encoding: 'utf8', windowsHide: true });
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
});
