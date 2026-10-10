import { existsSync, realpathSync } from 'node:fs';
import { resolve } from 'node:path';
import { root, run, requireBackend } from './backend.mjs';

try {
  requireBackend();
  const gitRoot = run('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'] }).trim();
  if (realpathSync(gitRoot) !== realpathSync(root)) throw new Error('Hooks require an Iris checkout with its own Git repository.');
  for (const file of ['lefthook.yml', 'commitlint.config.cjs', '.gitmessage', 'tools/check-staged.py', 'node_modules/lefthook/bin/index.js']) {
    if (!existsSync(resolve(root, file))) throw new Error(`Missing ${file}; run make setup.`);
  }
  run(process.execPath, ['node_modules/lefthook/bin/index.js', 'install']);
  run('git', ['config', '--local', 'commit.template', '.gitmessage']);
  console.log('Installed repository-local Lefthook hooks and commit template.');
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
