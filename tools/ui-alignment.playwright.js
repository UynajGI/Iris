// Run against the local disposable review app after opening a photo:
// playwright-cli run-code --filename tools/ui-alignment.playwright.js
// This file is a CLI callback, not a production bundle or @playwright/test spec.
async page => {
  const group = page.getByRole('group', { name: '颜色标签', exact: true });
  await group.waitFor({ state: 'visible' });
  const originalViewport = page.viewportSize();
  const original = await page.evaluate(() => ({
    size: document.documentElement.getAttribute('data-size'),
    density: document.documentElement.getAttribute('data-density'),
    zoom: document.documentElement.style.zoom,
    colors: [...document.querySelectorAll('.color-button')].map(button => ({
      pressed: button.getAttribute('aria-pressed'), disabled: button.disabled,
    })),
  }));
  const noMotion = await page.addStyleTag({ content: '*,*::before,*::after{animation:none!important;transition:none!important}' });
  let cases = 0;
  let controls = 0;
  let worstOffset = 0;
  try {
    for (const [width, height] of [[1024, 720], [1366, 900], [1920, 1080]]) {
      await page.setViewportSize({ width, height });
      for (const size of ['small', 'medium', 'large']) {
        for (const density of ['compact', 'normal', 'relaxed']) {
          for (const zoom of [1, 1.25, 1.5]) {
            const result = await page.evaluate(({ size, density, zoom }) => {
              const root = document.documentElement;
              root.dataset.size = size;
              root.dataset.density = density;
              root.style.zoom = String(zoom);
              const errors = [];
              let checked = 0;
              let worst = 0;
              const near = (a, b, label) => {
                const delta = Math.abs(a - b);
                worst = Math.max(worst, delta);
                if (delta > 1) errors.push(`${label}: ${delta.toFixed(2)}px`);
              };
              const center = element => {
                const rect = element.getBoundingClientRect();
                return { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2, rect };
              };
              const buttons = [...document.querySelectorAll('.color-button')];
              // Six selected values plus the no-photo/disabled state, without API writes.
              for (let selected = -1; selected < buttons.length; selected++) {
                buttons.forEach((button, index) => {
                  button.disabled = selected === -1;
                  button.setAttribute('aria-pressed', String(index === selected));
                });
                const reference = center(buttons[0]);
                for (const button of buttons) {
                  const box = center(button);
                  const swatch = center(button.querySelector('.color-swatch,.color-empty-swatch'));
                  const label = button.getAttribute('aria-label');
                  near(box.y, reference.y, `${label} row center`);
                  near(box.rect.width, reference.rect.width, `${label} button width`);
                  near(box.rect.height, reference.rect.height, `${label} button height`);
                  near(swatch.x, box.x, `${label} swatch X`);
                  near(swatch.y, box.y, `${label} swatch Y`);
                  checked++;
                }
                const rowLabel = center(buttons[0].closest('.mark-row').querySelector('.mark-label'));
                near(rowLabel.y, reference.y, '颜色 label center');
              }
              // Shared icons: toolbar, sidebar, marks, panel headings and viewer controls.
              for (const icon of document.querySelectorAll('button > .icon')) {
                const button = icon.parentElement;
                const box = center(button);
                if (!box.rect.width || !box.rect.height || getComputedStyle(button).visibility === 'hidden') continue;
                const visual = center(icon);
                near(visual.y, box.y, `${button.className} icon Y`);
                if (button.matches('.icon-only,.star-button,.hud-button,.stage-nav')) near(visual.x, box.x, `${button.className} icon X`);
                checked++;
              }
              for (const row of document.querySelectorAll('.mark-row')) {
                const label = center(row.querySelector('.mark-label'));
                const control = center(row.children[1]);
                near(label.y, control.y, 'mark label/control center');
                for (const button of row.querySelectorAll('button')) {
                  const rect = button.getBoundingClientRect();
                  if (rect.right > control.rect.right + 1 || rect.left < control.rect.left - 1) errors.push(`${button.className}: overflows its control row`);
                }
              }
              return { errors, checked, worst };
            }, { size, density, zoom });
            if (result.errors.length) throw new Error(`${width}×${height}, ${size}, ${density}, zoom ${zoom}:\n${result.errors.slice(0, 12).join('\n')}`);
            controls += result.checked;
            worstOffset = Math.max(worstOffset, result.worst);
            cases++;
          }
        }
      }
    }
    return { cases, controls, worstOffsetPx: worstOffset, status: 'passed' };
  } finally {
    await page.evaluate(original => {
      const root = document.documentElement;
      for (const name of ['size', 'density']) {
        if (original[name] === null) root.removeAttribute(`data-${name}`);
        else root.setAttribute(`data-${name}`, original[name]);
      }
      root.style.zoom = original.zoom;
      document.querySelectorAll('.color-button').forEach((button, index) => {
        button.disabled = original.colors[index].disabled;
        button.setAttribute('aria-pressed', original.colors[index].pressed);
      });
    }, original);
    await noMotion.evaluate(element => element.remove());
    if (originalViewport) await page.setViewportSize(originalViewport);
  }
}
