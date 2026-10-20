import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import type { Adjustments } from '../src/editing-client.js';
import { colorBalance, defaultAdjustments, defaultColorBalance } from '../src/editing-document.js';
import { ColorWheels } from '../src/ui/color/ColorWheels.js';
import { WheelReadout } from '../src/ui/color/WheelReadout.js';

/** A document saved before the global wheel and range blending/balance existed. */
function legacyAdjustments(): Adjustments {
  return {
    ...defaultAdjustments(),
    color_balance: { shadows: [0, 0, 0], midtones: [0, 0, 0], highlights: [0, 0, 0] },
  } as unknown as Adjustments;
}
function grading(overrides: Partial<Adjustments['color_balance']>): Adjustments {
  return {
    ...defaultAdjustments(),
    color_balance: { ...defaultColorBalance(), ...overrides },
  };
}
function markup(adjustments: Adjustments) {
  return renderToStaticMarkup(
    createElement(ColorWheels, { adjustments, disabled: false, onChange: () => {} })
  );
}
function inputValue(html: string, label: string) {
  const match = new RegExp(`aria-label="${label}"[^>]*value="([^"]+)"`).exec(html);
  assert.ok(match, `${label} is rendered`);
  return match[1]!;
}

test('neutral grading extensions keep the legacy document comparable', () => {
  assert.deepEqual(defaultColorBalance(), {
    shadows: [0, 0, 0],
    midtones: [0, 0, 0],
    highlights: [0, 0, 0],
    global: [0, 0, 0],
    blending: 0.5,
    balance: 0,
  });
  assert.deepEqual(defaultAdjustments().color_balance, defaultColorBalance());
});

test('missing grading extensions read as renderer defaults without mutating the document', () => {
  const legacy = legacyAdjustments();
  const restored = colorBalance(legacy.color_balance);
  assert.deepEqual(restored.global, [0, 0, 0]);
  assert.equal(restored.blending, 0.5);
  assert.equal(restored.balance, 0);
  assert.deepEqual(legacy.color_balance, {
    shadows: [0, 0, 0],
    midtones: [0, 0, 0],
    highlights: [0, 0, 0],
  });
});

test('saved grading extensions survive returning to the panel', () => {
  const restored = colorBalance(
    grading({ global: [0.75, -0.5, 0.25], blending: 0.2, balance: -0.6 }).color_balance
  );
  assert.deepEqual(restored.global, [0.75, -0.5, 0.25]);
  assert.equal(restored.blending, 0.2);
  assert.equal(restored.balance, -0.6);
});

test('grading panel offers the global wheel, blending and balance for legacy drafts', () => {
  const html = markup(legacyAdjustments());
  for (const name of ['三轮', '阴影', '中间调', '高光', '全局'])
    assert.ok(html.includes(name), name);
  assert.equal(html.match(/class="grade-wheel-mini grade-wheel-editor"/g)?.length, 3);
  assert.ok(!html.includes('aria-label="全局色轮"'), 'Global has its own page');
  for (const name of ['阴影', '中间调', '高光']) {
    assert.ok(html.includes(`aria-label="${name}亮度调整"`));
    assert.ok(html.includes(`aria-label="${name} HSL 数值"`));
  }
  assert.equal(inputValue(html, '混合数值'), '50');
  assert.equal(inputValue(html, '平衡数值'), '0');
});

test('wheel readout matches numeric controls and uses compact H S L values', () => {
  const html = renderToStaticMarkup(
    createElement(WheelReadout, {
      label: '中间调',
      value: { hue: 248.6, saturation: 0.64, luminance: -0.26 },
      disabled: false,
      onChange: () => {},
    })
  );
  for (const [name, value] of [
    ['H', '249'],
    ['S', '32'],
    ['L', '-13'],
  ]) {
    assert.match(html, new RegExp(`aria-label="中间调 ${name}，点击输入"[^>]*>${value}</button>`));
  }
  assert.ok(!html.includes('<input'), 'readout stays text until the user starts editing');
  assert.ok(!html.includes('°'));
});

test('grading panel shows the saved blending and balance instead of neutral defaults', () => {
  const html = markup(grading({ blending: 0.25, balance: -0.4, global: [0.5, 0, 0] }));
  assert.equal(inputValue(html, '混合数值'), '25');
  assert.equal(inputValue(html, '平衡数值'), '-40');
  assert.ok(html.includes('>混合</label>'));
  assert.ok(html.includes('>平衡</label>'));
});
