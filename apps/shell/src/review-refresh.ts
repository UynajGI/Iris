// Opt-in to the local review harness only; production WebViews never poll for builds.
if (
  location.hostname === 'localhost' &&
  document.querySelector('script[src="/__review/bridge.js"]')
) {
  let revision: number | undefined;
  let checking = false;
  setInterval(() => {
    if (checking || document.hidden) return;
    checking = true;
    void fetch('/review-build.json', { cache: 'no-store', signal: AbortSignal.timeout(5000) })
      .then(async (response) => {
        if (!response.ok) return;
        const build = (await response.json()) as { revision?: number };
        if (typeof build.revision !== 'number') return;
        if (
          revision !== undefined &&
          build.revision !== revision &&
          document.documentElement.dataset.irisEditingDirty !== 'true'
        ) {
          location.reload();
        } else if (revision === undefined) revision = build.revision;
      })
      .catch(() => {})
      .finally(() => {
        checking = false;
      });
  }, 1000);
}
