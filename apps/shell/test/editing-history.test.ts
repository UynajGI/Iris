import { test } from 'node:test';
import assert from 'node:assert/strict';
import { historySummary } from '../src/editing-history.js';
test('history displays parameter names and the same units as sliders', () => {
  const entry = {
    revision: 1,
    action: 'save',
    created_at: '2026-10-20T00:00:00Z',
    changes: { '基础调整/parameters/exposure': '0.5', '基础调整/parameters/highlights': '-0.8' },
  };
  assert.equal(historySummary(entry), '曝光 0.5 EV，高光 -40');
});
test('history describes arrays and undo without leaking curve payloads', () => {
  assert.equal(
    historySummary({
      revision: 2,
      action: 'undo',
      created_at: '',
      changes: { '曲线/parameters/master_curve': '' },
    }),
    '撤销 · RGB 曲线'
  );
});
