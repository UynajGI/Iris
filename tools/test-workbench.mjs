// Local Chromium smoke test. Run against tools/preview-frontend.mjs, never a personal library.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, readFile, writeFile, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { callPageFunction } from './cdp-page.mjs';

const origin = 'http://localhost:1420';
const chrome =
  process.env.IRIS_TEST_CHROME ?? 'C:/Program Files/Google/Chrome/Application/chrome.exe';
const output = resolve(process.env.IRIS_TEST_OUTPUT ?? '.qwen/tmp/workbench-review');
const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
await access(chrome);
assert.equal(
  (await fetch(`${origin}/__review/status`)).status,
  200,
  'disposable preview harness required'
);
await mkdir(output, { recursive: true });
const profile = await mkdtemp(join(tmpdir(), 'iris-chromium-test-'));
const child = spawn(
  chrome,
  [
    '--headless=new',
    '--remote-debugging-port=0',
    `--user-data-dir=${profile}`,
    '--no-first-run',
    '--no-default-browser-check',
    '--window-size=1440,960',
    'about:blank',
  ],
  { stdio: 'ignore' }
);
let socket;
const results = [];
const exceptions = [];
let corrupt = '';
let holdMarks = false;
let taskFixture = null;
const heldMarks = [];
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
  assert.ok(port, 'Chrome debugging port');
  const pages = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  socket = new WebSocket(pages.find((page) => page.type === 'page').webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true });
    socket.addEventListener('error', reject, { once: true });
  });
  let sequence = 0;
  const pending = new Map();
  function send(method, params = {}) {
    return new Promise((resolve, reject) => {
      const id = ++sequence;
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`CDP timeout: ${method}`));
      }, 15000);
      pending.set(id, { resolve, reject, timer });
      socket.send(JSON.stringify({ id, method, params }));
    });
  }
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data);
    if (message.id) {
      const entry = pending.get(message.id);
      if (!entry) return;
      pending.delete(message.id);
      clearTimeout(entry.timer);
      if (message.error) entry.reject(new Error(message.error.message));
      else entry.resolve(message.result);
    } else if (message.method === 'Runtime.exceptionThrown') {
      exceptions.push(message.params.exceptionDetails.text);
    } else if (message.method === 'Fetch.requestPaused') {
      handlePausedRequest(message.params);
    }
  });
  function handlePausedRequest({ requestId, request }) {
    if (
      taskFixture &&
      request.method === 'GET' &&
      new URL(request.url).pathname.endsWith('/progress')
    ) {
      void send('Fetch.fulfillRequest', {
        requestId,
        responseCode: 200,
        responseHeaders: [
          { name: 'Content-Type', value: 'application/json' },
          { name: 'Access-Control-Allow-Origin', value: origin },
        ],
        body: Buffer.from(JSON.stringify(taskFixture)).toString('base64'),
      }).catch((error) => exceptions.push(error.message));
      return;
    }
    if (
      holdMarks &&
      request.method === 'POST' &&
      /\/(marks|decisions)$/.test(new URL(request.url).pathname)
    ) {
      heldMarks.push(requestId);
      return;
    }
    const replace = corrupt && new URL(request.url).pathname.endsWith(`/${corrupt}`);
    void send(
      replace ? 'Fetch.fulfillRequest' : 'Fetch.continueRequest',
      replace
        ? {
            requestId,
            responseCode: 200,
            responseHeaders: [
              { name: 'Content-Type', value: 'image/jpeg' },
              { name: 'Access-Control-Allow-Origin', value: origin },
            ],
            body: Buffer.from('invalid-image-fixture').toString('base64'),
          }
        : { requestId }
    ).catch((error) => exceptions.push(error.message));
  }
  async function evaluate(expression) {
    const response = await send('Runtime.evaluate', {
      expression,
      awaitPromise: true,
      returnByValue: true,
      userGesture: true,
    });
    if (response.exceptionDetails)
      throw new Error(
        response.exceptionDetails.exception?.description ?? response.exceptionDetails.text
      );
    return response.result.value;
  }
  async function eventually(predicate, timeout = 20000) {
    const until = Date.now() + timeout;
    while (Date.now() < until) {
      if (await predicate()) return true;
      await delay(100);
    }
    return false;
  }
  async function wait(expression, name = expression, timeout = 20000) {
    const predicate = typeof expression === 'function' ? expression : () => evaluate(expression);
    if (await eventually(predicate, timeout)) return;
    const state = await evaluate(
      '({ alerts: [...document.querySelectorAll(".error-bar")].map(n=>n.textContent), page: document.querySelector(".module[aria-current=page]")?.textContent, dialogs: [...document.querySelectorAll("dialog")].map(n=>({open:n.open,title:n.querySelector("h2")?.textContent})), group: document.querySelector(".subbar")?.textContent, loading: document.querySelector(".compare-stage")?.getAttribute("aria-busy"), cells: document.querySelectorAll(".compare-cell").length, task: document.querySelector(".task-strip")?.textContent, empty: document.querySelector(".canvas-empty")?.textContent })'
    );
    throw new Error(`Timed out: ${name}; ${JSON.stringify(state)}`);
  }
  async function click(selector) {
    await wait(
      () =>
        callPageFunction(
          send,
          function (selector) {
            const node = document.querySelector(selector);
            return !!node && !node.disabled;
          },
          selector
        ),
      `enabled ${selector}`
    );
    const point = await callPageFunction(
      send,
      function (selector) {
        const node = document.querySelector(selector);
        node.scrollIntoView({ block: 'nearest', inline: 'nearest' });
        const r = node.getBoundingClientRect();
        const x = r.x + r.width / 2,
          y = r.y + r.height / 2;
        const hit = document.elementFromPoint(x, y);
        if (!node.contains(hit))
          throw new Error('Click obstructed: ' + selector + ' by ' + hit?.className);
        return { x, y };
      },
      selector
    );
    await send('Input.dispatchMouseEvent', {
      type: 'mousePressed',
      button: 'left',
      clickCount: 1,
      ...point,
    });
    await send('Input.dispatchMouseEvent', {
      type: 'mouseReleased',
      button: 'left',
      clickCount: 1,
      ...point,
    });
  }
  async function key(value, modifiers = 0) {
    await send('Input.dispatchKeyEvent', {
      type: 'keyDown',
      key: value,
      modifiers,
      windowsVirtualKeyCode:
        { Tab: 9, Escape: 27, Enter: 13, ArrowRight: 39, ArrowLeft: 37 }[value] ??
        (value.length === 1 ? value.toUpperCase().charCodeAt(0) : 0),
    });
    await send('Input.dispatchKeyEvent', { type: 'keyUp', key: value, modifiers });
  }
  async function shot(name) {
    if (process.env.IRIS_TEST_CAPTURE === '0') return;
    await evaluate('document.fonts.ready');
    const { data } = await send('Page.captureScreenshot', { format: 'png' });
    await writeFile(join(output, `${name}.png`), Buffer.from(data, 'base64'));
  }
  async function check(name, run) {
    if (process.env.IRIS_TEST_CHECKS && !process.env.IRIS_TEST_CHECKS.split('|').includes(name))
      return;
    await run();
    results.push(name);
    console.log(`PASS ${name}`);
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
  await wait('!!document.querySelector(".projects-heading")');
  await evaluate(`(() => {
    const invoke = window.__TAURI__.core.invoke;
    window.__TAURI__.core.invoke = function(command, args) {
      if (command === 'set_unsaved_changes') window.__workbenchUnsavedChanges = args.dirty;
      return invoke.call(this, command, args);
    };
  })()`);
  await shot('01-projects');
  await click('.projects-heading .primary');
  await wait('!!document.querySelector("dialog[open] .path-box")');
  assert.match(
    await evaluate('document.querySelector("dialog .path-box").textContent'),
    /iris-ui-review-/,
    'only disposable authorized copies'
  );
  await click('dialog[open] .dialog-actions .primary');
  await wait(
    'document.querySelectorAll(".cell-photo img.loaded").length >= 3',
    'scanned thumbnails',
    60000
  );
  await wait('!document.querySelector(".analyze-button").disabled', 'scan finished', 60000);
  // Select the fixture explicitly instead of assuming scanning focuses a photo.
  await click('.cell:first-child');
  await check('desktop layout and histogram', async () => {
    await wait('!!document.querySelector(".histogram-curves")');
    assert.ok(await evaluate('document.documentElement.scrollWidth <= innerWidth'));
    assert.ok(
      await evaluate('document.querySelector(".canvas").getBoundingClientRect().height > 300')
    );
    await shot('02-grid');
  });
  await check('project start page stays minimal and recent projects remain usable', async () => {
    await click('.modules .module:first-child');
    await wait('!!document.querySelector(".projects-heading")');
    assert.equal(
      await evaluate('document.querySelector(".projects-heading h1").textContent'),
      '项目'
    );
    assert.equal(
      await evaluate(
        'document.querySelectorAll(".hero, .hero-sub, .hero-steps, .project-text .muted, .current-badge").length'
      ),
      0
    );
    assert.ok(
      await evaluate('document.querySelector(".project-open").title.includes("iris-ui-review-")'),
      'folder path is available on demand'
    );
    await shot('28-minimal-projects');
    await send('Emulation.setDeviceMetricsOverride', {
      width: 800,
      height: 560,
      deviceScaleFactor: 1,
      mobile: false,
    });
    assert.ok(await evaluate('document.documentElement.scrollWidth<=innerWidth'));
    await shot('29-minimal-projects-compact');
    await send('Emulation.setDeviceMetricsOverride', {
      width: 1440,
      height: 960,
      deviceScaleFactor: 1,
      mobile: false,
    });
    await click('.project-open[aria-current=true]');
    await wait('document.querySelectorAll(".cell").length>=3');
  });
  await check(
    'mark toolbar stays above filmstrip, outside sidebar and available when collapsed',
    async () => {
      assert.equal(
        await evaluate('document.querySelectorAll(".sidebar-right .mark-tools").length'),
        0
      );
      assert.equal(
        await evaluate('document.querySelectorAll(".filmstrip-head .mark-tools").length'),
        1
      );
      assert.equal(await evaluate('document.querySelectorAll(".mark-undo").length'), 0);
      const singleRow =
        '(() => {const nodes=[...document.querySelectorAll(".mark-tools .decision-control, .mark-tools .star-control, .mark-tools .color-control, .gallery-scale, .filter-anchor")];const centers=nodes.map(n=>{const r=n.getBoundingClientRect();return r.top+r.height/2;});return Math.max(...centers)-Math.min(...centers)<2;})()';
      const aligned =
        '(() => {const bar=document.querySelector(".mark-tools").getBoundingClientRect(),film=document.querySelector(".filmstrip").getBoundingClientRect();return bar.bottom<=film.top && document.documentElement.scrollWidth<=innerWidth;})()';
      assert.ok(await evaluate(aligned));
      assert.ok(await evaluate(singleRow), 'desktop tools share one row');
      await shot('15-bottom-marks');
      await click('[aria-label="收起胶片栏"]');
      assert.equal(await evaluate('document.querySelectorAll(".filmstrip").length'), 0);
      if (await evaluate('document.querySelectorAll(".star-button[aria-pressed=true]").length===3'))
        await click('.star-button[aria-label="3 星"]');
      await click('.star-button[aria-label="3 星"]');
      assert.equal(
        await evaluate(
          'document.querySelector(".star-button[aria-label=\\"3 星\\"]").getAttribute("aria-pressed")'
        ),
        'true'
      );
      await click('[aria-label="展开胶片栏"]');
      await evaluate('document.querySelector(".cell-check input").click()');
      await wait('!!document.querySelector(".filmstrip-head .bulk-fields")');
      await shot('16-bottom-bulk');
      await click('.bulk-actions .quiet');
      await click('.view-tab[title="放大查看 (E)"]');
      await wait('!!document.querySelector(".zoom-image.loaded")');
      assert.ok(await evaluate(aligned));
      await shot('17-bottom-loupe');
      await key('g');
      await send('Emulation.setDeviceMetricsOverride', {
        width: 962,
        height: 700,
        deviceScaleFactor: 1,
        mobile: false,
      });
      assert.ok(await evaluate(singleRow), '678px workspace tools share one row');
      await shot('18-bottom-single-row');
      await send('Emulation.setDeviceMetricsOverride', {
        width: 800,
        height: 560,
        deviceScaleFactor: 1,
        mobile: false,
      });
      assert.ok(await evaluate(aligned));
      assert.ok(await evaluate(singleRow), '536px workspace tools share one row');
      assert.ok(
        await evaluate(
          '(() => {const bar=document.querySelector(".filmstrip-head").getBoundingClientRect();return [...document.querySelectorAll(".mark-tools button")].every(n=>{const r=n.getBoundingClientRect();return r.left>=bar.left && r.right<=bar.right && r.bottom<=bar.bottom;});})()'
        )
      );
      await shot('18-bottom-compact');
      await send('Emulation.setDeviceMetricsOverride', {
        width: 1440,
        height: 960,
        deviceScaleFactor: 1,
        mobile: false,
      });
    }
  );
  await check(
    'completed tasks stay quiet, details open above canvas and failures remain actionable',
    async () => {
      assert.equal(
        await evaluate('document.querySelectorAll(".taskbar").length'),
        0,
        'completed scan does not need dismissing'
      );
      const height = await evaluate(
        'document.querySelector(".canvas").getBoundingClientRect().height'
      );
      await click('[aria-label="任务"]');
      await wait('!!document.querySelector(".task-center.open .taskbar-done")');
      assert.equal(
        await evaluate('document.querySelector(".canvas").getBoundingClientRect().height'),
        height,
        'details never reflow workspace'
      );
      assert.equal(
        await evaluate('document.querySelectorAll(".taskbar progress").length'),
        0,
        'completed task is a record, not a full progress bar'
      );
      await shot('24-task-details');
      await key('Escape');
      assert.equal(await evaluate('document.activeElement.getAttribute("aria-label")'), '任务');
      await click('[aria-label="任务"]');
      await click('.view-tab[title="网格 (G)"]');
      assert.equal(
        await evaluate('!!document.querySelector(".task-center.open")'),
        false,
        'outside click closes panel'
      );
      taskFixture = {
        id: 'ui-failure-fixture',
        kind: 'analysis',
        state: 'failed',
        completed: 12,
        total: 12,
        failed_photo_ids: [1],
        failed_scan_paths: [],
        root_unavailable: false,
        errors: ['测试照片读取失败'],
        execution: [
          { phase: 'quality', selected_provider: 'cpu', completed_items: 11, warnings: [] },
        ],
        previous_execution: [],
        result: null,
      };
      await send('Fetch.enable', {
        patterns: [{ urlPattern: '*/progress', requestStage: 'Request' }],
      });
      await click('.filter-anchor > button');
      await evaluate(
        '(() => {const s=document.querySelectorAll(".filter-popover select")[4];s.value="jpeg";s.dispatchEvent(new Event("change",{bubbles:true}));})()'
      );
      await wait('!!document.querySelector(".taskbar-attention")');
      assert.ok(
        await evaluate(
          'document.querySelector(".taskbar-attention").textContent.includes("重试失败的 1 张")'
        )
      );
      await click('.popover-actions .primary');
      await shot('25-task-failure');
      await click('.taskbar [aria-label="关闭提示"]');
      await click('[aria-label="任务"]');
      assert.ok(
        await evaluate('!!document.querySelector(".task-center.open .taskbar-attention")'),
        'dismissed issue is still recoverable'
      );
      assert.ok(
        await evaluate('document.querySelector(".task-center").textContent.includes("计算设备")'),
        'reported execution details are available on demand'
      );
      await key('Escape');
      taskFixture = null;
      await send('Fetch.disable');
      await click('.filter-anchor > button');
      await click('.popover-actions .quiet');
      await click('.popover-actions .primary');
      await wait(
        'document.querySelectorAll(".taskbar").length===0 && document.querySelectorAll(".cell").length===12'
      );
      await shot('26-quiet-workspace');
    }
  );
  await check(
    'gallery zoom reflows, transitions to loupe and returns; filters stay below',
    async () => {
      const slider = async (value) =>
        evaluate(
          `(() => { const input=document.querySelector('.gallery-scale input'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set.call(input,${value}); input.dispatchEvent(new Event('input',{bubbles:true})); })()`
        );
      assert.equal(await evaluate('!!document.querySelector(".toolbar .filter-anchor")'), false);
      assert.equal(
        await evaluate('!!document.querySelector(".filmstrip-head .filter-anchor")'),
        true
      );
      await slider(140);
      await wait(
        'parseInt(document.querySelector(".grid").style.getPropertyValue("--tile"))===140'
      );
      await wait('document.querySelector(".cell").getAnimations().length === 0');
      const small = await evaluate('document.querySelector(".cell").getBoundingClientRect().width');
      await slider(280);
      assert.ok(
        await evaluate(
          '[...document.querySelectorAll(".cell")].some(n=>n.getAnimations().length>0)'
        ),
        'reflow animates'
      );
      await wait('document.querySelector(".cell").getAnimations().length === 0');
      assert.ok(
        (await evaluate('document.querySelector(".cell").getBoundingClientRect().width')) >
          small * 1.8
      );
      const id = await evaluate('document.querySelector(".cell.current").dataset.photoId');
      await slider(660);
      assert.ok(
        await evaluate('!!document.querySelector(".gallery-flight")'),
        'same photo moves into loupe'
      );
      await wait(
        '!!document.querySelector(".zoom-image.loaded") && !document.querySelector(".gallery-flight")'
      );
      assert.equal(
        await evaluate(
          'document.querySelector(".gallery-scale input").getAttribute("aria-valuetext")'
        ),
        '单图'
      );
      await shot('20-gallery-loupe');
      await slider(300);
      assert.ok(
        await evaluate('!!document.querySelector(".gallery-flight")'),
        'same photo returns to grid'
      );
      await wait('!!document.querySelector(".grid") && !document.querySelector(".gallery-flight")');
      assert.equal(await evaluate('document.querySelector(".cell.current").dataset.photoId'), id);
      await slider(660);
      await slider(260);
      await slider(660);
      await slider(240);
      assert.ok(await evaluate('document.querySelectorAll(".gallery-flight").length<=1'));
      await wait(
        '!document.querySelector(".gallery-flight") && !document.querySelector(".gallery-arriving")'
      );
      const point = await evaluate(
        '(() => {const r=document.querySelector(".canvas").getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};})()'
      );
      await send('Input.dispatchMouseEvent', {
        type: 'mouseWheel',
        ...point,
        deltaX: 0,
        deltaY: 150,
      });
      await wait('document.querySelector(".grid").scrollTop>0');
      assert.ok(
        await evaluate('!!document.querySelector(".grid")'),
        'plain wheel scrolls, not zooms'
      );
      await send('Input.dispatchMouseEvent', {
        type: 'mouseWheel',
        ...point,
        deltaX: 0,
        deltaY: -150,
        modifiers: 2,
      });
      await send('Input.dispatchMouseEvent', {
        type: 'mouseWheel',
        ...point,
        deltaX: 0,
        deltaY: -150,
        modifiers: 2,
      });
      await wait(
        '!!document.querySelector(".zoom-image.loaded") && !document.querySelector(".gallery-flight")'
      );
      await click('[aria-label="缩小"]');
      await wait('!!document.querySelector(".grid") && !document.querySelector(".gallery-flight")');
      async function checkReducedMotionAndFilters() {
        await send('Emulation.setEmulatedMedia', {
          features: [{ name: 'prefers-reduced-motion', value: 'reduce' }],
        });
        await slider(660);
        assert.equal(await evaluate('!!document.querySelector(".gallery-flight")'), false);
        await slider(210);
        assert.equal(
          await evaluate(
            '[...document.querySelectorAll(".cell")].some(n=>n.getAnimations().length>0)'
          ),
          false
        );
        await send('Emulation.setEmulatedMedia', { features: [] });
        await send('Emulation.setDeviceMetricsOverride', {
          width: 800,
          height: 560,
          deviceScaleFactor: 1,
          mobile: false,
        });
        await click('.filter-anchor > button');
        assert.ok(
          await evaluate(
            '(() => {const r=document.querySelector(".filter-popover").getBoundingClientRect(),b=document.querySelector(".filter-anchor").getBoundingClientRect(),w=document.querySelector(".workspace").getBoundingClientRect();return r.top>=w.top && r.left>=0 && r.right<=innerWidth && r.bottom<=b.top;})()'
          )
        );
        await shot('21-bottom-filters-compact');
        await evaluate(
          '(() => {const s=document.querySelectorAll(".filter-popover select")[4];s.value="raw";s.dispatchEvent(new Event("change",{bubbles:true}));})()'
        );
        await wait('!!document.querySelector(".canvas-empty")');
        assert.ok(
          await evaluate('!!document.querySelector(".filmstrip-head .filter-anchor")'),
          'empty result keeps filter recovery reachable'
        );
        await click('.popover-actions .quiet');
        await wait('document.querySelectorAll(".cell").length===12');
        await click('.popover-actions .primary');
        assert.ok(await evaluate('document.documentElement.scrollWidth<=innerWidth'));
        await send('Emulation.setDeviceMetricsOverride', {
          width: 1440,
          height: 960,
          deviceScaleFactor: 1,
          mobile: false,
        });
        await wait(
          '!!document.querySelector(".histogram-curves") && document.querySelectorAll(".filmstrip img.loaded").length>=3'
        );
        await shot('22-gallery-bottom-tools');
      }
      await checkReducedMotionAndFilters();
    }
  );
  await check('small thumbnails keep stars, color and format inside the card', async () => {
    await evaluate('document.activeElement?.blur()');
    await key('g');
    if (
      !(await evaluate(
        'document.querySelector("[aria-label=\\"显示照片信息\\"]").getAttribute("aria-pressed")==="true"'
      ))
    )
      await click('[aria-label="显示照片信息"]');
    await key('5');
    await click('.color-purple');
    const slider = async (value) =>
      evaluate(
        `(() => { const input=document.querySelector('.gallery-scale input'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set.call(input,${value}); input.dispatchEvent(new Event('input',{bubbles:true})); })()`
      );
    const contained =
      '(() => {const cell=document.querySelector(".cell.current"),box=cell.getBoundingClientRect(),meta=cell.querySelector(".cell-meta"),stars=cell.querySelector(".stars").getBoundingClientRect(),tags=cell.querySelector(".cell-tags").getBoundingClientRect();return [...cell.querySelectorAll(".cell-meta, .filename-line, .stars, .cell-tags, .color-dot, .cell-format")].every(node=>{const r=node.getBoundingClientRect();return r.left>=box.left && r.right<=box.right && r.bottom<=box.bottom;}) && meta.scrollWidth<=meta.clientWidth && Math.abs(stars.top+stars.height/2-tags.top-tags.height/2)<2;})()';
    for (const size of [140, 160, 210, 380]) {
      await slider(size);
      await wait('document.querySelector(".cell.current").getAnimations().length===0');
      if (!(await evaluate(contained))) {
        console.log(
          'CARD_BOUNDS',
          await evaluate(
            '(() => {const cell=document.querySelector(".cell.current");return [...cell.querySelectorAll(".cell-meta, .filename-line, .stars, .cell-tags, .color-dot, .cell-format"),cell].map(n=>({node:n.className,width:n.getBoundingClientRect().width,left:n.getBoundingClientRect().left,right:n.getBoundingClientRect().right,bottom:n.getBoundingClientRect().bottom,scroll:n.scrollWidth,client:n.clientWidth}));})()'
          )
        );
        await shot('23-overflow-failure');
      }
      assert.ok(await evaluate(contained), `metadata stays within ${size}px tile`);
    }
    await slider(140);
    await wait('document.querySelector(".cell.current").getAnimations().length===0');
    await evaluate(
      'document.querySelector(".cell.current .filename-line").textContent="很长的照片文件名_20261011_婚礼照片_第000001张.JPG"'
    );
    await evaluate('document.documentElement.dataset.size="large"');
    assert.ok(await evaluate(contained), 'long filename and large type stay inside smallest tile');
    assert.equal(
      await evaluate('document.querySelector(".cell.current .cell-format").textContent'),
      'JPEG'
    );
    await shot('23-small-card-contained');
    await evaluate('delete document.documentElement.dataset.size');
    await slider(210);
  });
  await check('Tab traverses controls, B toggles panels', async () => {
    await evaluate('document.querySelector(".tool-button").focus()');
    await key('Tab');
    assert.equal(await evaluate('document.activeElement.classList.contains("view-tab")'), true);
    assert.equal(await evaluate('document.querySelectorAll(".workspace > aside").length'), 2);
    await key('b');
    assert.equal(await evaluate('document.querySelectorAll(".workspace > aside").length'), 0);
    await key('b');
    assert.equal(await evaluate('document.querySelectorAll(".workspace > aside").length'), 2);
    await click('.sidebar-right .panel-toggle');
    assert.equal(
      await evaluate('document.querySelector(".sidebar-right .panel-body").hidden'),
      true
    );
    await click('.sidebar-right .panel-toggle');
    assert.equal(
      await evaluate('document.querySelector(".sidebar-right .panel-body").hidden'),
      false
    );
    await key('?', 8);
    await wait('!!document.querySelector("dialog[open]")');
    await key('b');
    assert.equal(await evaluate('document.querySelectorAll(".workspace > aside").length'), 2);
    await key('Escape');
    await wait('!document.querySelector("dialog[open]")');
  });
  await check(
    'scope modes replace one plot, channels overlay and clipping labels align',
    async () => {
      await wait(
        'document.querySelectorAll(".histogram-curves path").length === 4',
        'histogram ready after reopening inspector'
      );
      await click('.scope-channel.channel-l');
      assert.equal(await evaluate('document.querySelectorAll(".histogram-curves path").length'), 3);
      await click('.scope-channel.channel-r');
      await click('.scope-channel.channel-g');
      await click('.scope-channel.channel-b');
      await click('.scope-channel.channel-l');
      assert.equal(await evaluate('document.querySelectorAll(".histogram-curves path").length'), 1);
      assert.ok(await evaluate('!!document.querySelector(".histogram-curves .hist-l")'));
      await evaluate(
        '(() => {const s=document.querySelector("[aria-label=\\"示波器类型\\"]");s.value="waveform";s.dispatchEvent(new Event("change",{bubbles:true}));})()'
      );
      await wait('!!document.querySelector("canvas.scope-waveform")');
      assert.equal(await evaluate('document.querySelectorAll(".scope-plot").length'), 1);
      assert.equal(await evaluate('document.querySelectorAll(".histogram-curves").length'), 0);
      assert.ok(
        await evaluate(
          '(() => {const row=document.querySelector(".histogram-meta"),a=row.firstElementChild.getBoundingClientRect(),b=row.lastElementChild.getBoundingClientRect(),r=row.getBoundingClientRect();return Math.abs(a.left-r.left)<1 && Math.abs(b.right-r.right)<1;})()'
        )
      );
      await shot('14-waveform');
      await evaluate(
        '(() => {const s=document.querySelector("[aria-label=\\"示波器类型\\"]");s.value="histogram";s.dispatchEvent(new Event("change",{bubbles:true}));})()'
      );
      await click('.scope-channel.channel-r');
      await click('.scope-channel.channel-g');
      await click('.scope-channel.channel-b');
    }
  );
  await check('mark feedback precedes network completion and permits rapid changes', async () => {
    await send('Fetch.enable', {
      patterns: ['*/marks', '*/decisions'].map((urlPattern) => ({
        urlPattern,
        requestStage: 'Request',
      })),
    });
    holdMarks = true;
    const started = Date.now();
    assert.equal(
      await evaluate('!!document.querySelector(".star-clear, .decision-pending")'),
      false
    );
    assert.equal(await evaluate('!!document.querySelector(".color-none")'), true);
    const wasFlagged = await evaluate(
      'document.querySelector(".decision-flag").getAttribute("aria-pressed")'
    );
    await click('.decision-flag');
    assert.equal(
      await evaluate('document.querySelector(".decision-flag").getAttribute("aria-pressed")'),
      wasFlagged === 'true' ? 'false' : 'true'
    );
    await click('.decision-flag');
    assert.equal(
      await evaluate('document.querySelector(".decision-flag").getAttribute("aria-pressed")'),
      wasFlagged
    );
    assert.equal(
      await evaluate(
        'getComputedStyle(document.querySelector(".decision-flag")).transitionDuration'
      ),
      '0s'
    );
    // Normalize any persisted two-star fixture without needing a clear button.
    if (await evaluate('document.querySelectorAll(".star-button[aria-pressed=true]").length === 2'))
      await click('.star-button[aria-label="2 星"]');
    await click('.star-button[aria-label="2 星"]');
    assert.equal(
      await evaluate(
        'document.querySelector(".star-button[aria-label=\\"2 星\\"]").getAttribute("aria-pressed")'
      ),
      'true'
    );
    await click('.star-button[aria-label="5 星"]');
    await click('.star-button[aria-label="5 星"]');
    assert.equal(
      await evaluate('document.querySelectorAll(".star-button[aria-pressed=true]").length'),
      0
    );
    await click('.star-button[aria-label="5 星"]');
    await click('.color-button.color-blue');
    assert.equal(
      await evaluate(
        'document.querySelector(".star-button[aria-label=\\"5 星\\"]").getAttribute("aria-pressed")'
      ),
      'true'
    );
    assert.equal(
      await evaluate(
        'document.querySelector(".color-button.color-blue").getAttribute("aria-pressed")'
      ),
      'true'
    );
    assert.ok(
      await eventually(() => heldMarks.length > 0),
      'first mark request reached the network'
    );
    assert.equal(heldMarks.length, 1, 'only first serialized request has reached the network');
    assert.equal(
      await evaluate('document.querySelector(".mark-tools").getAttribute("aria-busy")'),
      'false'
    );
    assert.equal(
      await evaluate('document.querySelectorAll(".mark-tools button:disabled").length'),
      0
    );
    console.log(
      `MARK_FEEDBACK decision/star toggles and color visible with response still held (${Date.now() - started}ms including automation, not UI latency)`
    );
    holdMarks = false;
    await send('Fetch.continueRequest', { requestId: heldMarks.shift() });
    await send('Fetch.disable');
    await shot('19-toggle-marks');
  });
  await check('bulk selection cannot leak into loupe', async () => {
    await evaluate('document.querySelector(".cell-check input").click()');
    await wait('!!document.querySelector(".bulk-fields")');
    await shot('03-selection');
    await click('.view-tab[title="放大查看 (E)"]');
    await wait('!!document.querySelector(".zoom-image.loaded")');
    assert.equal(await evaluate('!!document.querySelector(".bulk-fields")'), false);
    await key('a', 2);
    assert.equal(await evaluate('!!document.querySelector(".bulk-fields")'), false);
  });
  await check('pointer wheel zoom, pan, 1:1 and fit', async () => {
    const r = await evaluate(
      '(() => { const r=document.querySelector(".zoom-stage").getBoundingClientRect(); return {x:r.x+r.width*.6,y:r.y+r.height*.6}; })()'
    );
    await send('Input.dispatchMouseEvent', { type: 'mouseWheel', ...r, deltaX: 0, deltaY: -650 });
    await wait('document.querySelector(".zoom-stage").classList.contains("zoomed")');
    const before = await evaluate('document.querySelector(".zoom-image").style.transform');
    await send('Input.dispatchMouseEvent', {
      type: 'mousePressed',
      button: 'left',
      clickCount: 1,
      ...r,
    });
    await send('Input.dispatchMouseEvent', {
      type: 'mouseMoved',
      button: 'left',
      buttons: 1,
      x: r.x + 75,
      y: r.y + 45,
    });
    await send('Input.dispatchMouseEvent', {
      type: 'mouseReleased',
      button: 'left',
      clickCount: 1,
      x: r.x + 75,
      y: r.y + 45,
    });
    assert.notEqual(
      await evaluate('document.querySelector(".zoom-image").style.transform'),
      before
    );
    await click('[aria-label="实际像素"]');
    await wait('document.querySelector(".zoom-readout").textContent === "100%"');
    await shot('04-loupe-actual');
    await click('[aria-label="适合窗口"]');
    await wait('document.querySelector(".zoom-readout").textContent === "适合"');
    await shot('05-loupe-fit');
  });
  await check('fullscreen enters and exits', async () => {
    await click('[aria-label="全屏"]');
    await wait('!!document.fullscreenElement');
    await shot('06-fullscreen');
    await click('[aria-label="退出全屏"]');
    await wait('!document.fullscreenElement');
  });
  await check(
    'double click animates zoom and restore; reduced motion skips spatial animation',
    async () => {
      const point = await evaluate(
        '(() => {const r=document.querySelector(".zoom-stage").getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};})()'
      );
      const doubleClick = async () => {
        await send('Input.dispatchMouseEvent', {
          type: 'mousePressed',
          button: 'left',
          clickCount: 2,
          ...point,
        });
        await send('Input.dispatchMouseEvent', {
          type: 'mouseReleased',
          button: 'left',
          clickCount: 2,
          ...point,
        });
      };
      await doubleClick();
      assert.ok(
        await evaluate('document.querySelector(".zoom-stage").classList.contains("zoom-animating")')
      );
      await wait('!document.querySelector(".zoom-stage").classList.contains("zoom-animating")');
      assert.ok(
        await evaluate('document.querySelector(".zoom-stage").classList.contains("zoomed")')
      );
      await doubleClick();
      assert.ok(
        await evaluate('document.querySelector(".zoom-stage").classList.contains("zoom-animating")')
      );
      await wait('document.querySelector(".zoom-readout").textContent === "适合"');
      await send('Emulation.setEmulatedMedia', {
        features: [{ name: 'prefers-reduced-motion', value: 'reduce' }],
      });
      await doubleClick();
      assert.ok(
        await evaluate(
          '!document.querySelector(".zoom-stage").classList.contains("zoom-animating") && document.querySelector(".zoom-stage").classList.contains("zoomed")'
        )
      );
      await doubleClick();
      await send('Emulation.setEmulatedMedia', { features: [] });
    }
  );
  await check('filters, keyboard editing and reset', async () => {
    await key('g');
    await click('.tool-pill');
    await wait('!!document.querySelector(".filter-popover select")');
    const before = await evaluate('document.querySelectorAll(".workspace > aside").length');
    await key('b');
    assert.equal(await evaluate('document.querySelectorAll(".workspace > aside").length'), before);
    await shot('07-filters');
    await key('Escape');
    await wait('!document.querySelector(".filter-popover")');
  });
  await check('real analysis and comparison preserve fullscreen on writes', async () => {
    await click('.analyze-button');
    await wait('document.querySelector(".analyze-button").disabled', 'analysis started', 20000);
    assert.ok(
      await evaluate('!!document.querySelector(".taskbar-running progress")'),
      'live progress stays available'
    );
    await wait('!document.querySelector(".analyze-button").disabled', 'analysis finished', 120000);
    await wait(
      'document.querySelectorAll(".taskbar").length===0',
      'successful analysis collapses automatically'
    );
    await click('[aria-label="任务"]');
    assert.ok(await evaluate('!!document.querySelector(".task-center.open .taskbar-done")'));
    await shot('27-analysis-details');
    await key('Escape');
    await wait(
      '!!document.querySelector(".view-tab:last-child:not(:disabled)")',
      'fresh comparison groups'
    );
    await click('.view-tab[title="相似组比较"]');
    await wait('!!document.querySelector(".compare-cell:not(:disabled)")');
    await shot('08-compare');
    await click('[aria-label="全屏"]');
    await wait('!!document.fullscreenElement');
    await key('p');
    await wait(
      'document.querySelector(".compare-cell[data-focused=true]")?.textContent.includes("已标记")',
      'group flag applied'
    );
    assert.ok(await evaluate('!!document.fullscreenElement'));
    await click('[aria-label="退出全屏"]');
    await click('.subbar > .sub-button');
    await wait('!!document.querySelector(".grid")');
  });
  await check(
    'selection evidence and original face comparison at desktop and compact sizes',
    async () => {
      await click('.view-tab[title="相似组比较"]');
      await wait(
        'document.querySelectorAll(".compare-reasons li").length > 0',
        'shared group evidence'
      );
      await wait('!!document.querySelector(".selection-reasons")', 'photo evidence');
      await click('.compare-options button:last-child');
      await wait(
        'document.querySelectorAll(".face-detail svg image").length >= 2',
        'aligned face regions'
      );
      await wait(
        '[...document.querySelectorAll(".detail-source")].every(n=>n.textContent.includes("原图局部"))',
        'original face detail',
        60000
      );
      const identity = await evaluate(
        'document.querySelector(".compare-cell .compare-caption .filename-line").textContent'
      );
      const before = await evaluate(
        'document.querySelector(".face-detail svg").getAttribute("viewBox")'
      );
      await evaluate(
        '(() => {const s=document.querySelector(".face-detail select");if(s.options.length>1){s.value="1";s.dispatchEvent(new Event("change",{bubbles:true}));}})()'
      );
      if (await evaluate('document.querySelector(".face-detail select").options.length > 1')) {
        await wait(
          `document.querySelector('.face-detail svg').getAttribute('viewBox') !== ${JSON.stringify(before)}`,
          'different selected face region'
        );
      }
      assert.equal(
        await evaluate(
          'document.querySelector(".compare-cell .compare-caption .filename-line").textContent'
        ),
        identity
      );
      await shot('30-selection-face-detail');
      await send('Emulation.setDeviceMetricsOverride', {
        width: 800,
        height: 560,
        deviceScaleFactor: 1,
        mobile: false,
      });
      await wait('!document.querySelector(".sidebar-left")');
      assert.ok(await evaluate('document.documentElement.scrollWidth <= innerWidth'));
      assert.ok(
        await evaluate(
          '[...document.querySelectorAll(".face-detail-frame")].every(n=>n.getBoundingClientRect().height >= 80)'
        )
      );
      assert.ok(
        await evaluate(`[...document.querySelectorAll('.compare-cell')].every(cell => {
        const detail=cell.querySelector('.face-detail').getBoundingClientRect();
        const caption=cell.querySelector('.compare-caption').getBoundingClientRect();
        const frame=cell.querySelector('.face-detail-frame').getBoundingClientRect();
        const source=cell.querySelector('.detail-source').getBoundingClientRect();
        return frame.bottom <= source.top + 1 && detail.bottom <= caption.top + 1;
      })`),
        'compact face detail must not overlap its caption'
      );
      await shot('31-selection-face-compact');
      await send('Emulation.setDeviceMetricsOverride', {
        width: 1440,
        height: 960,
        deviceScaleFactor: 1,
        mobile: false,
      });
      await click('.compare-options button:first-child');
      await wait('document.querySelectorAll(".compare-frame img.loaded").length >= 2');
      await shot('32-selection-evidence');
      await click('.subbar > .sub-button');
      await wait('!!document.querySelector(".grid")');
    }
  );
  await check('rating, image identity and face overlay follow the current photo', async () => {
    const oldPreview = await evaluate('document.querySelector(".navigator-frame img")?.src');
    await click('.cell:nth-child(5) .cell-photo');
    await wait('!!document.querySelector(".histogram-curves")');
    assert.notEqual(
      await evaluate('document.querySelector(".navigator-frame img")?.src'),
      oldPreview
    );
    await key('e');
    await wait('!!document.querySelector(".zoom-image.loaded")');
    await key('4');
    await wait('!!document.querySelector(".hud-info .stars[aria-label=\\"4 星\\"]")');
    await click('[aria-label="人脸与眼部标注"]');
    await wait('document.querySelectorAll(".face-box").length > 0');
    await click('[aria-label="实际像素"]');
    await wait(
      '!!document.querySelector(".zoom-image.loaded") && !!document.querySelector(".face-layer")',
      'original image and overlay ready'
    );
    await wait('!document.querySelector(".zoom-stage").classList.contains("zoom-animating")');
    await wait(
      '!document.querySelector(".zoom-badge") && !!document.querySelector(".zoom-image.loaded") && !!document.querySelector(".face-layer")',
      'original pixels and overlay settled'
    );
    assert.ok(
      await evaluate(
        '(() => {const a=document.querySelector(".face-layer").getBoundingClientRect(),b=document.querySelector(".zoom-image").getBoundingClientRect();return ["x","y","width","height"].every(key=>Math.abs(a[key]-b[key])<1);})()'
      )
    );
    await shot('12-faces');
    await key('g');
    await wait('!!document.querySelector(".grid")');
  });
  await check('800×560 workspace keeps canvas usable', async () => {
    await send('Emulation.setDeviceMetricsOverride', {
      width: 800,
      height: 560,
      deviceScaleFactor: 1,
      mobile: false,
    });
    await wait('!document.querySelector(".sidebar-left")');
    assert.ok(await evaluate('document.documentElement.scrollWidth <= innerWidth'));
    assert.ok(
      await evaluate('document.querySelector(".canvas").getBoundingClientRect().height >= 200')
    );
    await shot('09-compact');
    await click('[aria-label="照片库面板"]');
    await wait('!!document.querySelector(".sidebar-left")');
    await click('[aria-label="关闭照片库面板"]');
    await wait('!document.querySelector(".sidebar-left")');
  });
  await check('large type, relaxed spacing and reduced motion', async () => {
    await send('Emulation.setDeviceMetricsOverride', {
      width: 1440,
      height: 960,
      deviceScaleFactor: 1,
      mobile: false,
    });
    await click('.module:last-child');
    await wait('!!document.querySelector(".settings-page")');
    await click('[aria-label="字号"] button:last-child');
    await click('[aria-label="界面密度"] button:last-child');
    await shot('10-settings');
    await click('input[aria-label="人脸置信度"]');
    await key('ArrowRight');
    await wait(
      '!![...document.querySelectorAll("button")].find(node=>node.textContent==="保存设置" && !node.disabled)',
      'settings draft changed'
    );
    await wait('window.__workbenchUnsavedChanges === true', 'unsaved-change guard registered');
    await click('.module:nth-child(2)');
    await wait('!!document.querySelector("dialog[open]")');
    assert.equal(await evaluate('document.querySelector("dialog h2").textContent'), '离开设置？');
    await click('dialog .dialog-actions button:first-child');
    assert.equal(await evaluate('!!document.querySelector(".settings-page")'), true);
    await click('.module:nth-child(2)');
    await wait('!!document.querySelector("dialog[open]")');
    await click('dialog .dialog-actions button:nth-child(2)');
    await wait('!document.querySelector("dialog[open]")');
    await send('Emulation.setEmulatedMedia', {
      features: [{ name: 'prefers-reduced-motion', value: 'reduce' }],
    });
    await click('.module:nth-child(2)');
    await wait('!!document.querySelector(".workspace")');
    assert.ok(await evaluate('document.documentElement.scrollWidth <= innerWidth'));
    assert.equal(
      await evaluate('getComputedStyle(document.querySelector(".cell")).transitionDuration'),
      '0s'
    );
    await shot('11-large-type');
    await send('Emulation.setDeviceMetricsOverride', {
      width: 800,
      height: 560,
      deviceScaleFactor: 1,
      mobile: false,
    });
    assert.ok(await evaluate('document.documentElement.scrollWidth <= innerWidth'));
    await shot('13-compact-large');
    await send('Emulation.setDeviceMetricsOverride', {
      width: 1440,
      height: 960,
      deviceScaleFactor: 1,
      mobile: false,
    });
  });
  await check('decode errors show preview fallback and never a permanent loader', async () => {
    await send('Fetch.enable', {
      patterns: [{ urlPattern: '*/photos/*', requestStage: 'Request' }],
    });
    corrupt = 'original';
    await key('e');
    await wait('!!document.querySelector(".zoom-image.loaded")');
    await click('[aria-label="实际像素"]');
    await wait('document.querySelector(".zoom-badge")?.textContent.includes("原图暂不可用")');
    assert.ok(await evaluate('!!document.querySelector(".zoom-image.loaded")'));
    await key('g');
    corrupt = 'preview';
    // Reload to clear the in-memory preview cache before injecting a decode failure.
    await send('Page.reload', { ignoreCache: true });
    await wait('!!document.querySelector(".projects-heading")');
    await wait('!!document.querySelector(".project-open")');
    await click('.project-open');
    await wait('document.querySelectorAll(".cell").length > 0');
    await click('.cell:first-child');
    await key('e');
    await wait('!!document.querySelector(".zoom-failed")');
    assert.equal(await evaluate('!!document.querySelector(".zoom-loading")'), false);
    corrupt = '';
    await send('Fetch.disable');
    await key('g');
    await key('e');
    await wait('!!document.querySelector(".zoom-image.loaded")');
  });
  assert.deepEqual(exceptions, [], 'browser runtime exceptions');
  await writeFile(
    join(output, 'results.json'),
    JSON.stringify(
      {
        passed: results,
        runtimeExceptions: exceptions,
        screenshotsCaptured: process.env.IRIS_TEST_CAPTURE !== '0',
      },
      null,
      2
    )
  );
  console.log(
    `RESULT ${results.length} browser checks passed; report: ${output}${process.env.IRIS_TEST_CAPTURE === '0' ? '; screenshot capture disabled' : '; screenshots captured'}`
  );
} catch (error) {
  console.error(error.message);
  await writeFile(
    join(output, 'failure.json'),
    JSON.stringify(
      { passed: results, error: error.message, runtimeExceptions: exceptions },
      null,
      2
    )
  );
  process.exitCode = 1;
} finally {
  socket?.close();
  child.kill();
}
