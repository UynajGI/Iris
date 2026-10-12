// Real browser display timings against the disposable public-data review harness.
// Start preview-frontend.mjs with IRIS_REVIEW_DATASET=ppr10k, COUNT=541 and a release daemon.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, writeFile, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { callPageFunction } from './cdp-page.mjs';
const output = resolve(process.argv[2] ?? 'artifacts/performance-browser.json');
const origin = 'http://localhost:1420';
assert.equal(
  (await fetch(origin + '/__review/status')).status,
  200,
  'disposable review harness required'
);
const profile = await mkdtemp(join(tmpdir(), 'iris-performance-browser-'));
const chrome = spawn(
  process.env.IRIS_TEST_CHROME ?? 'C:/Program Files/Google/Chrome/Application/chrome.exe',
  [
    '--headless=new',
    '--remote-debugging-port=0',
    `--user-data-dir=${profile}`,
    '--no-first-run',
    '--no-default-browser-check',
    '--window-size=1440,960',
    'about:blank',
  ],
  { stdio: 'ignore', windowsHide: true }
);
let socket;
const report = {
  public_fixtures_only: true,
  native_shell_measured: false,
  viewport: [1440, 960],
  timing: 'action to decoded image plus two animation frames',
};
const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
function stats(values) {
  const a = [...values].sort((x, y) => x - y);
  return {
    n: a.length,
    median_ms: a[Math.floor(a.length / 2)],
    p95_ms: a[Math.ceil(a.length * 0.95) - 1],
    max_ms: a.at(-1),
  };
}
try {
  let port;
  for (let i = 0; i < 100; i++) {
    try {
      port = Number((await readFile(join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0]);
      break;
    } catch {
      await delay(100);
    }
  }
  assert.ok(port);
  const pages = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  socket = new WebSocket(pages.find((page) => page.type === 'page').webSocketDebuggerUrl);
  await new Promise((yes, no) => {
    socket.addEventListener('open', yes, { once: true });
    socket.addEventListener('error', no, { once: true });
  });
  let sequence = 0;
  const pending = new Map();
  socket.addEventListener('message', ({ data }) => {
    const message = JSON.parse(data);
    if (message.id) {
      const item = pending.get(message.id);
      if (item) {
        clearTimeout(item.timer);
        pending.delete(message.id);
        message.error ? item.no(new Error(message.error.message)) : item.yes(message.result);
      }
    }
  });
  const send = (method, params = {}) =>
    new Promise((yes, no) => {
      const id = ++sequence;
      const timer = setTimeout(() => {
        pending.delete(id);
        no(new Error('CDP timeout: ' + method));
      }, 120000);
      pending.set(id, { yes, no, timer });
      socket.send(JSON.stringify({ id, method, params }));
    });
  const page = (fn, ...args) => callPageFunction(send, fn, ...args);
  async function wait(selector, timeout = 60000) {
    await page(
      async (selector, timeout) => {
        const start = performance.now();
        while (!document.querySelector(selector)) {
          if (performance.now() - start > timeout) throw new Error('Missing ' + selector);
          await new Promise((resolve) => setTimeout(resolve, 20));
        }
      },
      selector,
      timeout
    );
  }
  async function click(selector) {
    await wait(selector);
    await page((selector) => document.querySelector(selector).click(), selector);
  }
  await send('Runtime.enable');
  await send('Page.enable');
  await send('Emulation.setDeviceMetricsOverride', {
    width: 1440,
    height: 960,
    deviceScaleFactor: 1,
    mobile: false,
  });
  await send('Page.navigate', { url: origin });
  await wait('.projects-heading');
  report.page_start_ms = await page(() => performance.now());
  const cached = await page(() => !!document.querySelector('.project-card'));
  if (!cached) {
    await click('.projects-heading .primary');
    await wait('dialog[open] .path-box');
  }
  report[cached ? 'cached_open' : 'fresh_import_display'] = await page(async (cached) => {
    const start = performance.now();
    document
      .querySelector(cached ? '.project-open' : 'dialog[open] .dialog-actions .primary')
      .click();
    let first = null,
      firstScreen = null;
    const painted = () =>
      new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    while (performance.now() - start < 60000) {
      const count = document.querySelectorAll('.cell-photo img.loaded').length;
      if (count && first === null) {
        await painted();
        first = performance.now() - start;
      }
      if (count >= 12) {
        await painted();
        firstScreen = performance.now() - start;
        break;
      }
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
    if (firstScreen === null) throw new Error('First 12 thumbnails did not display');
    return {
      first_thumbnail_ms: first,
      first_12_thumbnails_ms: firstScreen,
      scan_running_at_first_screen: !!document.querySelector('.analyze-button')?.disabled,
    };
  }, cached);
  await page(async () => {
    const start = performance.now();
    while (document.querySelector('.analyze-button')?.disabled) {
      if (performance.now() - start > 60000) throw new Error('Scan did not finish');
      await new Promise((resolve) => setTimeout(resolve, 25));
    }
  });
  await click('.cell:first-child');
  await click('.view-tab[title="放大查看 (E)"]');
  await wait('.zoom-stage img');
  async function switches(count, direction) {
    return page(
      async (count, direction) => {
        const samples = [];
        for (let i = 0; i < count; i++) {
          const old = document.querySelector('.filename-line')?.textContent;
          const start = performance.now();
          document.querySelector(direction > 0 ? '.stage-next' : '.stage-prev').click();
          while (performance.now() - start < 10000) {
            const image = document.querySelector('.loupe img:not(.zoom-preload)');
            if (
              document.querySelector('.filename-line')?.textContent !== old &&
              image?.complete &&
              image.naturalWidth &&
              image.classList.contains('loaded')
            )
              break;
            await new Promise((resolve) => requestAnimationFrame(resolve));
          }
          if (performance.now() - start >= 10000) throw new Error('Photo switch timed out');
          await new Promise((resolve) =>
            requestAnimationFrame(() => requestAnimationFrame(resolve))
          );
          samples.push(performance.now() - start);
        }
        return samples;
      },
      count,
      direction
    );
  }
  report.switch_fresh = stats(await switches(30, 1));
  report.switch_cached = stats(await switches(30, -1));
  await page(() => {
    window.__selectionLongTasks = [];
    window.__selectionObserver = new PerformanceObserver((list) =>
      window.__selectionLongTasks.push(...list.getEntries().map((e) => e.duration))
    );
    window.__selectionObserver.observe({ type: 'longtask' });
  });
  await click('.analyze-button');
  report.switch_during_analysis = stats(await switches(30, 1));
  report.mark_display_ms = await page(async () => {
    const start = performance.now();
    const button = document.querySelector('.star-button[aria-label="3 星"]');
    if (!button) throw new Error('Rating control unavailable');
    const old = button.getAttribute('aria-pressed');
    button.click();
    while (button.getAttribute('aria-pressed') === old) {
      if (performance.now() - start > 5000) throw new Error('Rating did not update');
      await new Promise((resolve) => requestAnimationFrame(resolve));
    }
    await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    return performance.now() - start;
  });
  const state = await page(async () => {
    const session = await window.__TAURI__.core.invoke('daemon_session');
    const response = await fetch(session.base_url + '/api/v1/projects', {
      headers: { Authorization: 'Bearer ' + session.token },
    });
    const projects = await response.json();
    const headers = { Authorization: 'Bearer ' + session.token };
    const url = session.base_url + `/api/v1/projects/${projects[0].id}`;
    const active = await (await fetch(url + '/progress', { headers })).json();
    let final = active;
    const started = performance.now();
    while (['running', 'paused'].includes(final.state)) {
      if (performance.now() - started > 180000) throw new Error('Analysis timeout');
      await new Promise((resolve) => setTimeout(resolve, 200));
      final = await (await fetch(url + '/progress', { headers })).json();
    }
    const photos = await (await fetch(url + '/photos', { headers })).json();
    return { count: photos.length, active_after_switches: active.state, final };
  });
  report.library_count = state.count;
  report.analysis_active_after_switches = state.active_after_switches;
  report.analysis_result = state.final;
  report.long_tasks_during_analysis = await page(() => {
    window.__selectionObserver.disconnect();
    return window.__selectionLongTasks;
  });
  assert.ok(report.library_count >= 500, 'At least 500 actual public portraits');
  report.ok = true;
} catch (error) {
  report.error = error.message;
  throw error;
} finally {
  socket?.close();
  chrome.kill();
  await mkdir(resolve(output, '..'), { recursive: true });
  await writeFile(output, JSON.stringify(report, null, 2) + '\n');
}
console.log(JSON.stringify(report));
