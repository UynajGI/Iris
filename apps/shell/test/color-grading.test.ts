import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import type { Adjustments } from '../src/editing-client.js';
import { colorBalance, defaultAdjustments, defaultColorBalance } from '../src/editing-document.js';
import { ColorWheels } from '../src/ui/color/ColorWheels.js';

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
  for (const name of ['全部', '阴影', '中间调', '高光', '全局'])
    assert.ok(html.includes(name), name);
  assert.equal(html.match(/class="grade-wheel-mini"/g)?.length, 4);
  assert.ok(html.includes('aria-label="全局色轮"'));
  assert.equal(inputValue(html, '混合数值'), '50');
  assert.equal(inputValue(html, '平衡数值'), '0');
});

test('grading panel shows the saved blending and balance instead of neutral defaults', () => {
  const html = markup(grading({ blending: 0.25, balance: -0.4, global: [0.5, 0, 0] }));
  assert.equal(inputValue(html, '混合数值'), '25');
  assert.equal(inputValue(html, '平衡数值'), '-40');
  assert.ok(html.includes('>混合</label>'));
  assert.ok(html.includes('>平衡</label>'));
});
