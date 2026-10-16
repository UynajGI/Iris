import { readFile, readdir, mkdir, copyFile, writeFile } from 'node:fs/promises';

/** Collect installed runtime notices from the lockfile; never fetch licenses during a build. */
export async function copyRuntimeLicenses() {
  const base = new URL('./', import.meta.url);
  const lock = JSON.parse(await readFile(new URL('package-lock.json', base), 'utf8'));
  const target = new URL('dist/licenses/npm/', base);
  await mkdir(target, { recursive: true });
  const index = [];
  for (const [location, entry] of Object.entries(lock.packages)) {
    if (!location || entry.dev) continue;
    const directory = new URL(`${location}/`, base);
    const name = location.replace(/^node_modules\//, '');
    const files = (await readdir(directory)).filter((file) => /^licen[cs]e(?:\.|$)/i.test(file));
    const sources = files.map((file) => new URL(file, directory));
    // npm 2.3.8 omits its MIT text; preserved from the author's upstream repository.
    if (name === 'react-remove-scroll-bar' && !sources.length)
      sources.push(new URL('src/ui/licenses/react-remove-scroll-bar.txt', base));
    if (!sources.length) throw new Error(`Missing runtime license: ${name}`);
    for (const [number, source] of sources.entries()) {
      const file = `${name.replaceAll('/', '__')}__${number}.txt`;
      await copyFile(source, new URL(file, target));
      index.push(`${name}@${entry.version} | ${entry.license} | ${file}`);
    }
  }
  await writeFile(new URL('index.txt', target), `${index.join('\n')}\n`);
}
