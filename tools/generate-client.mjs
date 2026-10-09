import { createRequire } from 'node:module';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';

// Keep generator dependencies scoped to the frontend package.
const require = createRequire(new URL('../apps/shell/package.json', import.meta.url));
const { default: openapiTS, astToString } = require('openapi-typescript');
const source = process.argv[2];
if (!source) throw new Error('Usage: npm run generate:api -- <exported-openapi.json>');
const schema = JSON.parse((await readFile(resolve(source), 'utf8')).replace(/^\uFEFF/, ''));
if (!String(schema.openapi).startsWith('3.')) throw new Error('Expected an OpenAPI 3 document');
if (!schema.paths?.['/api/v1/bootstrap']) throw new Error('Expected the Iris daemon contract');
const output = new URL('../apps/shell/src/generated/', import.meta.url);
await mkdir(output, { recursive: true });
const hash = createHash('sha256').update(JSON.stringify(schema)).digest('hex');
const content = `// Generated from iris-daemon OpenAPI. Do not edit.\n// Contract SHA-256: ${hash}\n` + astToString(await openapiTS(schema));
await writeFile(new URL('api.d.ts', output), content);
console.log(`Generated ${fileURLToPath(new URL('api.d.ts', output))}`);
