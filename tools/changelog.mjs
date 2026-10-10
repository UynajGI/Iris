// GPL-3.0-or-later. Generate changelogs locally; never tag, push or publish.
import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const cli = resolve(root, 'node_modules/git-cliff/lib/cli/cli.js');

export function generate(directory = root, { release = false } = {}) {
  const version = JSON.parse(readFileSync(resolve(directory, 'apps/shell/package.json'), 'utf8')).version;
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) throw new Error('Invalid release version');
  const git = (...args) => execFileSync('git', args, { cwd: directory, encoding: 'utf8' }).trim();
  if (git('rev-parse', '--is-shallow-repository') !== 'false') throw new Error('Fetch full Git history and tags before generating changelogs');
  const tag = `v${version}`;
  const tagged = git('tag', '--list', tag);
  if (tagged && git('rev-parse', `${tag}^{commit}`) !== git('rev-parse', 'HEAD')) {
    throw new Error('Bump the application version before generating a new release changelog');
  }
  const args = ['--config', resolve(root, 'cliff.toml')];
  if (!tagged) args.push('--tag', tag);
  if (release) args.push(tagged ? '--current' : '--unreleased', '--strip', 'header');
  return execFileSync(process.execPath, [cli, ...args], { cwd: directory, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 }).replaceAll('\r\n', '\n').trimEnd() + '\n';
}

export function releaseNotes(directory = root) {
  const version = JSON.parse(readFileSync(resolve(directory, 'apps/shell/package.json'), 'utf8')).version;
  const manual = readFileSync(resolve(directory, 'docs/releases', `v${version}.md`), 'utf8').trimEnd();
  const changes = generate(directory, { release: true }).trim();
  if (!changes.includes(`## v${version}`)) throw new Error('Generated changelog does not match the release version');
  return `${manual}\n\n---\n\n## Changes from Git history\n\n${changes}\n`;
}

function main(args) {
  const check = args.includes('--check');
  const release = args.includes('--release');
  const outputIndex = args.indexOf('--output');
  const output = outputIndex < 0 ? (release ? 'dist/release-notes.md' : 'CHANGELOG.md') : args[outputIndex + 1];
  const remaining = args.filter((_, i) => i !== outputIndex && !(outputIndex >= 0 && i === outputIndex + 1));
  if (!output || remaining.some(arg => !['--check', '--release'].includes(arg)) || (check && release)) throw new Error('Usage: changelog.mjs [--check | --release] [--output path]');
  const content = release ? releaseNotes() : generate();
  const path = resolve(root, output);
  if (check) {
    if (readFileSync(path, 'utf8').replaceAll('\r\n', '\n') !== content) throw new Error('CHANGELOG.md is stale; run make changelog before committing the release');
    console.log('Changelog matches full Git history and the application version');
  } else {
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, content, 'utf8');
    console.log(`Generated ${output}`);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try { main(process.argv.slice(2)); } catch (error) { console.error(error.message); process.exitCode = 1; }
}
