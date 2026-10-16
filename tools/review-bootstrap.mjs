// Prepare only the running review harness's disposable public fixture, never a user directory.
const origin = 'http://localhost:1420';
async function invoke(command) {
  const response = await fetch(`${origin}/__review/invoke`, {
    method: 'POST',
    headers: { Origin: origin, 'Content-Type': 'application/json' },
    body: JSON.stringify({ command }),
    signal: AbortSignal.timeout(10000),
  });
  if (!response.ok) throw new Error('The local review harness is unavailable');
  return response.json();
}
const session = await invoke('daemon_session');
const folder = await invoke('select_project_folder');
if (typeof folder !== 'string' || !/[\\/]iris-ui-review-[^\\/]+[\\/]photos$/.test(folder))
  throw new Error('Only a disposable review fixture is accepted');
const base = new URL(session.base_url);
if (base.protocol !== 'http:' || base.hostname !== '127.0.0.1')
  throw new Error('Invalid review daemon address');
async function request(method, path, body) {
  const response = await fetch(base.origin + '/api/v1' + path, {
    method,
    headers: {
      Authorization: `Bearer ${session.token}`,
      ...(body ? { 'Content-Type': 'application/json' } : {}),
    },
    ...(body ? { body: JSON.stringify(body) } : {}),
    signal: AbortSignal.timeout(30000),
  });
  if (!response.ok)
    throw new Error(`Review request failed: ${method} ${path} (${response.status})`);
  return response.json();
}
const project = await request('POST', '/projects', { root: folder, auto_device: false });
await request('POST', `/projects/${project.id}/scan`);
const deadline = Date.now() + 30000;
while (Date.now() < deadline) {
  const progress = await request('GET', `/projects/${project.id}/progress`);
  if (!['running', 'paused', 'cancelling'].includes(progress.state)) break;
  await new Promise((resolve) => setTimeout(resolve, 100));
}
const photos = await request('GET', `/projects/${project.id}/photos?limit=100`);
if (!photos.length) throw new Error('Review fixture contains no scanned photographs');
console.log(
  `Review project ${project.id}: ${photos.length} disposable public photographs. First photo ${photos[0].id}.`
);
