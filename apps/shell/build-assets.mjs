import { copyFile, mkdir, rm, readFile } from 'node:fs/promises';
import { build } from 'esbuild';
import { fileURLToPath } from 'node:url';
import { copyRuntimeLicenses } from './build-licenses.mjs';
const { version } = JSON.parse(await readFile(new URL('./package.json', import.meta.url), 'utf8'));
await copyRuntimeLicenses();
await mkdir(new URL('./dist/help/', import.meta.url), { recursive: true });
for (const [source, target] of [
  [`../../docs/releases/v${version}.md`, 'release-notes.txt'],
  ['../../LICENSE', 'LICENSE.txt'],
  ['../../THIRD_PARTY_NOTICES.md', 'THIRD_PARTY_NOTICES.txt'],
]) {
  await copyFile(
    new URL(source, import.meta.url),
    new URL(`./dist/help/${target}`, import.meta.url)
  );
}
// Remove historical test output from builds made with the development tsconfig.
await rm(new URL('./dist/test/', import.meta.url), { recursive: true, force: true });
await mkdir(new URL('./dist/src/generated/', import.meta.url), { recursive: true });
await copyFile(
  new URL('./src/generated/api.d.ts', import.meta.url),
  new URL('./dist/src/generated/api.d.ts', import.meta.url)
);
await copyFile(
  new URL('./index.html', import.meta.url),
  new URL('./dist/index.html', import.meta.url)
);
await build({
  entryPoints: [fileURLToPath(new URL('./src/main.tsx', import.meta.url))],
  outfile: fileURLToPath(new URL('./dist/app.js', import.meta.url)),
  bundle: true,
  format: 'esm',
  platform: 'browser',
  target: 'es2022',
  minify: true,
  legalComments: 'linked',
  loader: { '.woff2': 'file', '.svg': 'text', '.txt': 'text' },
  assetNames: 'assets/[name]-[hash]',
});
await mkdir(new URL('./dist/licenses/', import.meta.url), { recursive: true });
for (const name of ['noto-sans-sc', 'noto-serif-sc']) {
  await copyFile(
    new URL(`./node_modules/@fontsource-variable/${name}/LICENSE`, import.meta.url),
    new URL(`./dist/licenses/${name}-OFL.txt`, import.meta.url)
  );
}
await copyFile(
  new URL('./src/ui/icons/LICENSE', import.meta.url),
  new URL('./dist/licenses/material-symbols-LICENSE.txt', import.meta.url)
);
