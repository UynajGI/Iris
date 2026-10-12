// Fixed harness compiled only into the opt-in native-smoke feature.
// It exercises this application's entrypoint and IPC; no external desktop control.
(async () => {
  const invoke = window.__TAURI__.core.invoke;
  const waitFor = async (predicate, description) => {
    const deadline = Date.now() + 10000;
    while (!predicate()) {
      if (Date.now() > deadline) throw new Error(`Timeout: ${description}`);
      await new Promise((resolve) => setTimeout(resolve, 25));
    }
  };
  try {
    await waitFor(
      () => window.iris?.store.getSnapshot().bootstrap,
      'actual frontend entry initialization'
    );
    await waitFor(
      () => window.iris.updater?.getSnapshot().status,
      'native updater status initialization'
    );
    await waitFor(() => document.querySelector('.titlebar'), 'actual React interface');
    await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    if (window.iris.updater.getSnapshot().status.state !== 'not_configured') {
      throw new Error('Native smoke requires an unconfigured update source');
    }
    for (const [stage, language, title] of [
      ['english-before', 'en-US', 'Iris'],
      ['chinese', 'zh-CN', '伊人'],
      ['english-after', 'en-US', 'Iris'],
    ]) {
      // Exercise the real desktop languagechange handler, not a test reimplementation.
      Object.defineProperty(navigator, 'language', { configurable: true, value: language });
      window.dispatchEvent(new Event('languagechange'));
      await waitFor(() => document.title === title, `${stage} title synchronization`);
      await invoke('native_title_smoke_observe', { stage, documentTitle: document.title });
    }
    let rejected = false;
    try {
      await invoke('set_app_locale', { locale: 'arbitrary-window-title' });
    } catch {
      rejected = true;
    }
    if (!rejected) throw new Error('Native locale command accepted arbitrary text');
    await invoke('native_title_smoke_observe', {
      stage: 'invalid-rejected',
      documentTitle: document.title,
    });
    const action = (action) => invoke('native_window_smoke', { action });
    const assert = (value, message) => {
      if (!value) throw new Error(message);
    };
    const status = await action('show');
    assert(status.visible && status.tray, 'visible main window and recovery tray');
    assert(JSON.stringify(status.caption) === '[8,9,20]', 'native caption button hit targets');
    await action('maximize');
    assert((await action('status')).maximized, 'native maximize');
    await action('restore');
    await action('minimize');
    assert((await action('status')).minimized, 'native minimize');
    await action('show');
    assert(!(await action('status')).minimized, 'tray restore from minimized');
    await action('capture');
    await new Promise((resolve) => setTimeout(resolve, 2500));
    await invoke('set_close_behavior', { behavior: 'ask' });
    await action('close');
    await waitFor(
      () => document.querySelector('dialog[open] .close-remember'),
      'close choice dialog'
    );
    await action('dialog');
    await new Promise((resolve) => setTimeout(resolve, 2500));
    document.querySelector('.close-remember input').click();
    [...document.querySelectorAll('dialog[open] button')]
      .find((button) => button.textContent === '后台运行')
      .click();
    await waitFor(() => !document.querySelector('dialog[open]'), 'background dialog completed');
    assert(!(await action('status')).visible, 'background hides window');
    assert((await invoke('close_behavior')) === 'background', 'remembered background choice');
    await action('show');
    await invoke('set_unsaved_changes', { dirty: true });
    await action('close');
    await new Promise((resolve) => setTimeout(resolve, 100));
    assert(
      !(await action('status')).visible,
      'remembered background preserves drafts without asking'
    );
    await action('show');
    // The explicit tray action must bypass remembered background behavior.
    await action('tray-exit');
    await waitFor(
      () => document.querySelector('dialog[open]'),
      'tray exit request reaches frontend'
    );
    [...document.querySelectorAll('dialog[open] button')]
      .find((button) => button.textContent === '取消')
      .click();
    await invoke('set_unsaved_changes', { dirty: false });
    await invoke('set_close_behavior', { behavior: 'ask' });
    assert((await invoke('close_behavior')) === 'ask', 'reset close preference');
    await invoke('native_title_smoke_finish', { error: null });
  } catch (error) {
    await invoke('native_title_smoke_finish', {
      error: JSON.stringify({
        message: String(error),
        errors: window.__nativeTitleSmokeErrors,
        hasApplication: !!window.iris,
        readyState: document.readyState,
        origin: location.origin,
        scripts: [...document.scripts].map((script) => script.src),
      }),
    });
  }
})();
