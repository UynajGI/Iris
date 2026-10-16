import assert from 'node:assert/strict';
import test from 'node:test';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { Select } from '../src/ui/Select.js';

test('shared choice displays empty and numeric values, preserving labels and disabled state', () => {
  const options = [
    createElement('option', { key: '', value: '' }, '全部'),
    createElement('option', { key: 0, value: 0 }, '零星'),
    createElement('option', { key: 1, value: 1, disabled: true }, '不可用'),
  ];
  const render = (value: string | number, disabled = false) =>
    renderToStaticMarkup(
      createElement(Select, {
        value,
        disabled,
        'aria-label': '星级',
        onValueChange() {},
        children: options,
      })
    );
  assert.match(render(''), /role="combobox"[^>]*aria-label="星级"/);
  assert.match(render(''), />全部<\/span>/);
  assert.match(render(0), />零星<\/span>/);
  assert.match(render(1, true), /<button[^>]*disabled=""/);
  assert.match(render(1, true), />不可用<\/span>/);
});
