// Development-only rebuild loop; the running review harness keeps its dataset and daemon.
import { createRequire } from 'node:module';
import { writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
const require = createRequire(new URL('../apps/shell/package.json', import.meta.url));
const { context } = require('esbuild');
const shell = new URL('../apps/shell/', import.meta.url);
const build = await context({
  entryPoints: [fileURLToPath(new URL('src/main.tsx', shell))],
  outfile: fileURLToPath(new URL('dist/app.js', shell)),
  bundle: true,
  format: 'esm',
  platform: 'browser',
  target: 'es2022',
  minify: false,
  legalComments: 'linked',
  loader: { '.woff2': 'file', '.svg': 'text', '.txt': 'text' },
  assetNames: 'assets/[name]-[hash]',
  plugins: [
    {
      name: 'review-refresh',
      setup(builder) {
        builder.onEnd(async (result) => {
          if (result.errors.length) return;
          await writeFile(
            new URL('dist/review-build.json', shell),
            JSON.stringify({ revision: Date.now() })
          );
          console.log('Frontend rebuilt. Review refresh is ready.');
        });
      },
    },
  ],
});
await build.watch();
console.log('Watching frontend sources; no Rust build.');
async function stop() {
  await build.dispose();
  process.exit(0);
}
process.once('SIGINT', stop);
process.once('SIGTERM', stop);
