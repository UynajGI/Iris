// Local review harness: real frontend/API, disposable photos, explicit native-dialog substitutes.
// Never packaged. Start with `node tools/preview-frontend.mjs`; Ctrl+C stops its daemon.
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { mkdtemp, mkdir, readdir, copyFile, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve, extname, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const dist = join(root, 'apps/shell/dist');
const temporary = await mkdtemp(join(tmpdir(), 'iris-ui-review-'));
const photos = join(temporary, 'photos');
const models = join(temporary, 'models');
await mkdir(photos); await mkdir(models);
for (const name of (await readdir(join(root, 'test-photos'))).filter(name => /\.jpe?g$/i.test(name)).slice(0, 12)) {
  await copyFile(join(root, 'test-photos', name), join(photos, name));
}
for (const name of ['manifest.json', 'yunet.onnx', 'face_landmarks_detector.onnx', 'face_blendshapes.onnx', 'onnxruntime.dll', 'niqe_params.json']) {
  await copyFile(join(root, 'models', name), join(models, name));
}
let daemon;
let session;
let generation = 0;
async function startDaemon() {
const child = spawn(join(root, 'target/debug/iris-daemon.exe'), ['--data-dir', join(temporary, 'data'), '--model-dir', models, '--port', '0'], { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
daemon = child;
// Keep credentials in memory. No startup JSON or API token is logged.
child.stderr.resume();
child.once('exit', () => { if (daemon === child) session = null; });
session = await new Promise((accept, reject) => {
  const timeout = setTimeout(() => reject(new Error('Daemon startup timeout')), 30000);
  const lines = createInterface({ input: child.stdout });
  lines.once('line', line => { clearTimeout(timeout); lines.close(); try { accept(JSON.parse(line)); } catch { reject(new Error('Invalid daemon startup')); } });
  child.once('error', reject);
  child.once('exit', () => reject(new Error('Daemon stopped')));
}).catch(error => { child.kill(); throw error; });
generation += 1;
}
await startDaemon();
const bridge = `const reviewListeners = new Map();
let reviewGeneration;
setInterval(async () => {
  try {
    const response = await fetch('/__review/status');
    if (!response.ok) return;
    const current = await response.json();
    if (reviewGeneration !== undefined && current !== reviewGeneration) {
      for (const callback of reviewListeners.get(current === null ? 'daemon:unavailable' : 'daemon:restarted') ?? []) callback({ payload: null });
    }
    reviewGeneration = current;
  } catch {}
}, 1000);
window.__TAURI__ = {
  core: { async invoke(command, args) {
    const response = await fetch('/__review/invoke', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ command, args }) });
    if (!response.ok) throw new Error(await response.text());
    return response.json();
  } },
  event: { async listen(name, callback) {
    if (!reviewListeners.has(name)) reviewListeners.set(name, new Set());
    reviewListeners.get(name).add(callback);
    return () => reviewListeners.get(name).delete(callback);
  } }
};`;
const mime = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.css': 'text/css', '.woff2': 'font/woff2', '.svg': 'image/svg+xml' };
const server = createServer(async (request, response) => {
  response.setHeader('Cache-Control', 'no-store');
  response.setHeader('X-Content-Type-Options', 'nosniff');
  if (request.headers.host !== 'localhost:1420' || (request.headers.origin && request.headers.origin !== 'http://localhost:1420')) {
    response.writeHead(403).end(); return;
  }
  try {
    const url = new URL(request.url, 'http://localhost:1420');
    if (url.pathname === '/__review/status' && request.method === 'GET') {
      response.writeHead(200, { 'Content-Type': 'application/json' }).end(JSON.stringify(session ? generation : null)); return;
    }
    if (url.pathname === '/__review/invoke' && request.method === 'POST') {
      if (request.headers.origin !== 'http://localhost:1420' || request.headers['content-type'] !== 'application/json') { response.writeHead(403).end(); return; }
      let input = '';
      for await (const chunk of request) { input += chunk; if (input.length > 4096) throw new Error('Request too large'); }
      const { command } = JSON.parse(input);
      let value = null;
      switch (command) {
        case 'daemon_session': if (!session) throw new Error('Daemon unavailable'); value = session; break;
        case 'select_project_folder': {
          // Review-only folder picker: an optional local fixture file selects
          // a destination for export/cache tests, never an arbitrary user path.
          let selected = photos;
          try {
            const name = JSON.parse(await readFile(join(temporary, 'folder-selection.json'), 'utf8')).directory;
            if (!['photos', 'export', 'cache-moved'].includes(name)) throw new Error('Invalid fixture directory');
            selected = join(temporary, name);
            await mkdir(selected, { recursive: true });
          } catch (error) { if (error.code !== 'ENOENT') throw error; }
          value = selected; break;
        }
        case 'select_export_csv': value = join(temporary, 'decisions.csv'); break;
        case 'set_app_locale': value = '伊人'; break;
        case 'frontend_ready': case 'set_unsaved_changes': break;
        case 'update_status': value = { state: 'not_configured', current_version: '0.1.0', update: null, downloaded_bytes: 0, total_bytes: null, reason: '浏览器复核环境', error: null }; break;
        default: response.writeHead(409).end('此原生操作需要在 Tauri 中验证'); return;
      }
      response.writeHead(200, { 'Content-Type': 'application/json' }).end(JSON.stringify(value)); return;
    }
    if (request.method !== 'GET') { response.writeHead(405).end(); return; }
    if (url.pathname === '/__review/bridge.js') { response.writeHead(200, { 'Content-Type': mime['.js'] }).end(bridge); return; }
    const file = resolve(dist, '.' + decodeURIComponent(url.pathname === '/' ? '/index.html' : url.pathname));
    if (!file.startsWith(dist + sep)) { response.writeHead(403).end(); return; }
    let body = await readFile(file);
    if (extname(file) === '.html') body = Buffer.from(body.toString().replace('<script type="module"', '<script src="/__review/bridge.js"></script><script type="module"'));
    response.writeHead(200, { 'Content-Type': mime[extname(file)] ?? 'application/octet-stream' }).end(body);
  } catch { response.writeHead(500).end('Review fixture request failed'); }
});
server.once('error', error => { daemon.kill(); console.error(error.message); process.exitCode = 1; });
server.listen(1420, 'localhost', () => console.log(`Review: http://localhost:1420\nDisposable fixture: ${temporary}\nNative dialogs/events are substitutes; API operations are real.`));
function stop() { server.close(); daemon.kill(); controls.close(); }
process.on('SIGINT', stop); process.on('SIGTERM', stop);
// Local stdin only: exercise outage/reconnect without adding a browser control
// endpoint or changing the production application. Preserve the fixture database.
const controls = createInterface({ input: process.stdin });
let controlQueue = Promise.resolve();
controls.on('line', line => {
  controlQueue = controlQueue.then(async () => {
    if (line.trim() === 'stop-daemon') {
      if (daemon.exitCode === null && daemon.signalCode === null) {
        await new Promise(resolve => { daemon.once('exit', resolve); daemon.kill(); });
      }
      console.log('Review daemon stopped; browser server remains available.');
    } else if (line.trim() === 'start-daemon') {
      if (!session) { await startDaemon(); console.log('Review daemon restarted with the same fixture database.'); }
    }
  }).catch(error => console.error(error.message));
});
