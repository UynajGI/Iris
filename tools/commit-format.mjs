// Keep Lefthook's original index intact until it has restored unstaged changes.
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: 'utf8', ...options });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} failed (${result.status})`);
  return result.stdout;
}

function main() {
  const temporary = mkdtempSync(join(tmpdir(), 'iris-format-index-'));
  try {
    const index = resolve(run('git', ['rev-parse', '--git-path', 'index']).trim());
    const stagedIndex = join(temporary, 'index');
    copyFileSync(index, stagedIndex);
    const env = { ...process.env, GIT_INDEX_FILE: stagedIndex };
    const formatter = process.env.FORMATTER || 'tools/format.mjs';
    run(process.execPath, [formatter, '--staged'], { env, stdio: 'inherit' });
    run(process.env.MAKE || 'make', ['staged-check', 'quality-staged'], {
      env,
      stdio: 'inherit',
    });
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
}

try {
  main();
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
