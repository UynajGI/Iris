// Internal bridge for provisioning, validation and packaging. Public entry: Make.
import { existsSync } from 'node:fs';
import { delimiter, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

export const root = fileURLToPath(new URL('../', import.meta.url));
export const backend = process.env.IRIS_BACKEND || resolve(root, '.venv', process.platform === 'win32' ? 'Scripts/python.exe' : 'bin/python');
export function backendEnv() {
  const env = { ...process.env };
  const key = Object.keys(env).find(k => k.toLowerCase() === 'path') ?? 'PATH';
  env[key] = `${resolve(backend, '..')}${delimiter}${env[key] ?? ''}`;
  env.VIRTUAL_ENV = resolve(root, '.venv');
  return env;
}
export function run(command, args, options = {}) {
  const result = spawnSync(command, args, { cwd: root, stdio: 'inherit', ...options });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} failed (${result.status ?? result.signal})`);
  return result.stdout;
}
export function requireBackend() {
  if (!existsSync(backend)) throw new Error('Backend environment missing; run make env first.');
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    requireBackend();
    if (!process.argv[2]) throw new Error('Internal backend command required. Use make help.');
    run(backend, process.argv.slice(2), { env: backendEnv() });
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
