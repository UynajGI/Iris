// Platform-sensitive leaves only; task dependencies and commands live in Makefile.
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { backend, backendEnv, requireBackend, root, run } from './backend.mjs';

try {
  switch (process.argv[2]) {
    case 'help':
      for (const line of readFileSync(resolve(root, 'Makefile'), 'utf8').split('\n')) {
        const match = line.match(/^([\w-]+):.*?## (.+)$/);
        if (match) console.log(`${match[1].padEnd(23)} ${match[2]}`);
      }
      console.log('\nOptions: PROFILE=debug|release, OUTPUT="new path", ARGS="tool flags". Preview: make -n <target>.');
      console.log('Start: make setup. Optional downloads/indexing are separate explicit targets.');
      break;
    case 'env':
      if (!existsSync(backend)) {
        if (existsSync(resolve(root, '.venv'))) throw new Error('Existing .venv has no usable runtime; inspect it before recreating.');
        run(process.env.UV || 'uv', ['venv', '--python', '3.12', '.venv']);
      }
      run(backend, ['-c', 'import sys; assert sys.version_info >= (3,12), "Backend runtime requires 3.12+"']);
      console.log('Backend environment ready; no activation needed.');
      break;
    case 'doctor': {
      let failed = false;
      for (const [name, args] of [['git', ['--version']], ['cargo', ['--version']], ['rustc', ['--version']], ['node', ['--version']], [process.env.UV || 'uv', ['--version']]]) {
        try { run(name, args); } catch (error) { failed = true; console.error(error.message); }
      }
      console.log(`Backend environment: ${existsSync(backend) ? 'ready' : 'not prepared (make env)'}`);
      console.log('Native builds also require the platform-specific Tauri system prerequisites.');
      if (failed) process.exitCode = 1;
      break;
    }
    case 'require-output':
      if (!process.env.IRIS_OUTPUT?.trim()) throw new Error('Provide OUTPUT="new destination". Existing output is never overwritten.');
      break;
    case 'require-windows':
      if (process.platform !== 'win32') throw new Error('This provisioning target is Windows-only; see docs/media-support.md.');
      break;
    case 'portable':
      requireBackend();
      run('pwsh', ['-NoProfile', '-File', 'tools/package-local.ps1', '-Configuration', process.env.IRIS_PROFILE === 'release' ? 'Release' : 'Debug', '-OutputDirectory', process.env.IRIS_OUTPUT], { env: backendEnv() });
      break;
    default:
      throw new Error('Unknown internal operation; use make help.');
  }
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
