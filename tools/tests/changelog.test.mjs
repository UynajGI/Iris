import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { generate, releaseNotes } from '../changelog.mjs';

function fixture() {
  const directory = mkdtempSync(resolve(tmpdir(), 'iris-changelog-'));
  const git = (...args) => execFileSync('git', args, { cwd: directory, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], env: {
    ...process.env, GIT_AUTHOR_NAME: 'Iris Test', GIT_AUTHOR_EMAIL: 'test@example.invalid',
    GIT_COMMITTER_NAME: 'Iris Test', GIT_COMMITTER_EMAIL: 'test@example.invalid',
    GIT_AUTHOR_DATE: '2026-10-10T00:00:00Z', GIT_COMMITTER_DATE: '2026-10-10T00:00:00Z',
  } }).trim();
  git('init', '--quiet');
  mkdirSync(resolve(directory, 'apps/shell'), { recursive: true });
  writeFileSync(resolve(directory, 'apps/shell/package.json'), JSON.stringify({ version: '0.1.0-beta2' }));
  git('add', '.'); git('-c', 'commit.gpgsign=false', 'commit', '-qm', 'feat: first beta');
  git('tag', 'v0.1.0-beta');
  writeFileSync(resolve(directory, 'fix.txt'), 'regression');
  git('add', '.'); git('-c', 'commit.gpgsign=false', 'commit', '-qm', 'fix(security): reject linked model paths');
  return { directory, git };
}

test('virtual release equals real tag and release metadata commits do not cause drift', () => {
  const { directory, git } = fixture();
  try {
    const before = generate(directory);
    assert.match(before, /## v0\.1\.0-beta2/);
    assert.match(before, /### Fixes/);
    assert.match(before, /reject linked model paths/);
    writeFileSync(resolve(directory, 'CHANGELOG.md'), before);
    git('add', '.'); git('-c', 'commit.gpgsign=false', 'commit', '-qm', 'chore(release): prepare beta2');
    git('tag', 'v0.1.0-beta2');
    assert.equal(generate(directory), before);
    const latest = generate(directory, { release: true });
    assert.match(latest, /reject linked model paths/);
    assert.doesNotMatch(latest, /first beta|prepare beta2/);
  } finally { rmSync(directory, { recursive: true, force: true }); }
});

test('release notes retain explicit installation and support limits', () => {
  const { directory } = fixture();
  try {
    mkdirSync(resolve(directory, 'docs/releases'), { recursive: true });
    writeFileSync(resolve(directory, 'docs/releases/v0.1.0-beta2.md'), '# Beta2\n\nUnsigned installers; no automatic updates.\n');
    const notes = releaseNotes(directory);
    assert.match(notes, /Unsigned installers; no automatic updates/);
    assert.match(notes, /Changes from Git history/);
    assert.match(notes, /reject linked model paths/);
    assert.doesNotMatch(notes, /first beta/);
  } finally { rmSync(directory, { recursive: true, force: true }); }
});

test('shallow history and a reused release version fail closed', () => {
  const { directory, git } = fixture();
  const clone = mkdtempSync(resolve(tmpdir(), 'iris-shallow-'));
  try {
    git('tag', 'v0.1.0-beta2');
    writeFileSync(resolve(directory, 'next.txt'), 'next');
    git('add', '.'); git('-c', 'commit.gpgsign=false', 'commit', '-qm', 'fix: another change');
    assert.throws(() => generate(directory), /Bump the application version/);
    execFileSync('git', ['clone', '--depth', '1', pathToFileURL(directory).href, clone], { stdio: 'pipe' });
    assert.throws(() => generate(clone), /Fetch full Git history/);
  } finally { rmSync(directory, { recursive: true, force: true }); rmSync(clone, { recursive: true, force: true }); }
});
