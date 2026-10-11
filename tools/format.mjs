// Shared formatting entry; Lefthook hides partial changes before --staged runs.
import { spawnSync } from 'node:child_process';
import { lstatSync, readFileSync, writeFileSync } from 'node:fs';
import { extname, resolve } from 'node:path';
import * as prettier from 'prettier';
import toml from 'prettier-plugin-toml';
import xml from '@prettier/plugin-xml';

const root = process.cwd();
const plugins = [toml, xml];
const extensions = new Set([
  '.ts',
  '.tsx',
  '.js',
  '.jsx',
  '.mjs',
  '.cjs',
  '.json',
  '.jsonc',
  '.yml',
  '.yaml',
  '.md',
  '.mdx',
  '.css',
  '.scss',
  '.html',
  '.toml',
  '.svg',
  '.xml',
]);

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { cwd: root, encoding: 'utf8', ...options });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} failed (${result.status})`);
  return result.stdout;
}

async function files(staged) {
  const args = staged
    ? ['diff', '--cached', '--name-only', '--diff-filter=ACMR', '-z']
    : ['ls-files', '--cached', '--others', '--exclude-standard', '-z'];
  const names = [...new Set(run('git', args).split('\0').filter(Boolean))];
  const selected = [];
  for (const name of names) {
    const extension = extname(name);
    if (!extensions.has(extension) && !['.rs', '.py'].includes(extension)) continue;
    const info = await prettier.getFileInfo(resolve(root, name), {
      ignorePath: [resolve(root, '.prettierignore'), resolve(root, '.gitignore')],
      resolveConfig: false,
    });
    if (info.ignored) continue;
    if (!lstatSync(resolve(root, name)).isFile())
      throw new Error(`${name}: expected regular source`);
    selected.push(name);
  }
  return selected;
}

function python(names, check) {
  if (!names.length) return;
  const uv = process.env.UV || 'uv';
  const prefix = [
    'run',
    '--locked',
    '--inexact',
    '--default-index',
    'https://pypi.org/simple',
    'ruff',
  ];
  run(
    uv,
    [
      ...prefix,
      'check',
      '--no-cache',
      '--select',
      'I',
      ...(check ? [] : ['--fix', '--no-unsafe-fixes']),
      '--',
      ...names,
    ],
    { stdio: 'inherit' }
  );
  run(uv, [...prefix, 'format', ...(check ? ['--check'] : []), '--', ...names], {
    stdio: 'inherit',
  });
}

async function text(name, check) {
  const path = resolve(root, name);
  const source = readFileSync(path, 'utf8');
  let formatted;
  if (extname(name) === '.rs') {
    formatted = run(process.env.RUSTFMT || 'rustfmt', ['--emit', 'stdout', '--edition', '2021'], {
      input: source,
      stdio: ['pipe', 'pipe', 'inherit'],
    });
  } else {
    const options = await prettier.resolveConfig(path, { editorconfig: true });
    const parser =
      extname(name) === '.toml'
        ? 'toml'
        : ['.svg', '.xml'].includes(extname(name))
          ? 'xml'
          : undefined;
    try {
      formatted = await prettier.format(source, {
        ...options,
        plugins,
        filepath: path,
        ...(parser ? { parser } : {}),
      });
    } catch {
      // Syntax diagnostics can contain source text, including credentials.
      throw new Error(`${name}: formatting failed; fix the staged syntax`);
    }
  }
  if (source.replaceAll('\r\n', '\n') === formatted) return true;
  if (check) {
    console.error(`${name}: needs formatting`);
    return false;
  }
  writeFileSync(path, formatted);
  return true;
}

async function main() {
  const staged = process.argv.includes('--staged');
  const check = process.argv.includes('--check');
  const names = await files(staged);
  if (staged && names.length) {
    // Outside Lefthook, fail before writing if partially staged files are visible.
    run('git', ['diff', '--quiet', '--', ...names]);
  }
  python(
    names.filter((name) => extname(name) === '.py'),
    check
  );
  let valid = true;
  for (const name of names.filter((name) => extname(name) !== '.py')) {
    valid = (await text(name, check)) && valid;
  }
  if (!valid) throw new Error('Formatting check failed; run make fmt');
  if (staged && names.length) run('git', ['add', '--', ...names]);
  console.log(
    `Formatting: ${names.length} files ${check ? 'checked' : 'formatted'}${staged ? ' and staged' : ''}`
  );
}

main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
