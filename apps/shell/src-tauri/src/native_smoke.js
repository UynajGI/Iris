// Fixed harness compiled only into the opt-in native-smoke feature.
// It exercises this application's entrypoint and IPC; no external desktop control.
(async () => {
  const invoke = window.__TAURI__.core.invoke;
  const waitFor = async (predicate, description) => {
    const deadline = Date.now() + 10000;
    while (!predicate()) {
      if (Date.now() > deadline) throw new Error(`Timeout: ${description}`);
      await new Promise(resolve => setTimeout(resolve, 25));
    }
  };
  try {
    await waitFor(() => window.iris?.store.getSnapshot().bootstrap, 'actual frontend entry initialization');
    await waitFor(() => window.iris.updater?.getSnapshot().status, 'native updater status initialization');
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
    try { await invoke('set_app_locale', { locale: 'arbitrary-window-title' }); }
    catch { rejected = true; }
    if (!rejected) throw new Error('Native locale command accepted arbitrary text');
    await invoke('native_title_smoke_observe', { stage: 'invalid-rejected', documentTitle: document.title });
    await invoke('native_title_smoke_finish', { error: null });
  } catch (error) {
    await invoke('native_title_smoke_finish', { error: JSON.stringify({
      message: String(error), errors: window.__nativeTitleSmokeErrors,
      hasApplication: !!window.iris, readyState: document.readyState, origin: location.origin,
      scripts: [...document.scripts].map(script => script.src),
    }) });
  }
})();
