import { mkdtempSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { isDeepStrictEqual } from 'node:util';
import { root, run } from './backend.mjs';

export function verifySchema(raw, existing, check) {
  const schema = JSON.parse(raw.replace(/^\uFEFF/, ''));
  if (!schema.paths?.['/api/v1/bootstrap']) throw new Error('Unexpected API schema; existing files were not changed.');
  if (check && !isDeepStrictEqual(JSON.parse(existing.replace(/^\uFEFF/, '')), schema)) throw new Error('OpenAPI is stale; run make api and review changes.');
  return schema;
}
if (process.argv[1] && resolve(process.argv[1]) === resolve(root, 'tools/api.mjs')) {
  let temporary;
  try {
    const check = process.argv.includes('--check');
    const raw = run('cargo', ['run', '--locked', ...(process.env.IRIS_PROFILE === 'release' ? ['--release'] : []), '-p', 'iris-daemon', '--', '--openapi'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'], maxBuffer: 16 * 1024 * 1024 });
    const path = resolve(root, 'docs/openapi.json');
    const schema = verifySchema(raw, check ? readFileSync(path, 'utf8') : '', check);
    temporary = mkdtempSync(resolve(tmpdir(), 'iris-api-'));
    const generated = resolve(temporary, 'openapi.json');
    const text = JSON.stringify(schema, null, 2) + '\n';
    writeFileSync(generated, text);
    run(process.execPath, ['tools/generate-client.mjs', generated, ...(check ? ['--check'] : [])]);
    if (!check) writeFileSync(path, text);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  } finally {
    if (temporary) rmSync(temporary, { recursive: true });
  }
}
