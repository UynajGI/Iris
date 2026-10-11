import assert from 'node:assert/strict';
import test from 'node:test';
import { callPageFunction } from '../cdp-page.mjs';

function transport(callResult = { result: { value: 42 } }, releaseError = false) {
  const calls = [];
  return {
    calls,
    async send(method, params) {
      calls.push({ method, params });
      if (method === 'Runtime.evaluate') return { result: { objectId: 'page' } };
      if (method === 'Runtime.releaseObject') {
        if (releaseError) throw new Error('cleanup failed');
        return {};
      }
      if (callResult instanceof Error) throw callResult;
      return callResult;
    },
  };
}

test('selector values are data, never interpolated JavaScript', async () => {
  const selector = `</script>"'\\\n\u2028\u2029; throw new Error('injected')`;
  const cdp = transport();
  const callback = function (value) {
    return document.querySelector(value);
  };
  assert.equal(await callPageFunction(cdp.send, callback, selector), 42);
  const request = cdp.calls[1].params;
  assert.equal(request.functionDeclaration, Function.prototype.toString.call(callback));
  assert.ok(!request.functionDeclaration.includes(selector));
  assert.deepEqual(request.arguments, [{ value: selector }]);
  assert.equal(request.objectId, 'page');
  assert.equal(request.awaitPromise, true);
  assert.equal(request.returnByValue, true);
  assert.equal(request.userGesture, true);
  assert.deepEqual(cdp.calls[2], {
    method: 'Runtime.releaseObject',
    params: { objectId: 'page' },
  });
});

test('browser exceptions propagate and release the remote object', async () => {
  const cdp = transport({ exceptionDetails: { exception: { description: 'blocked' } } });
  await assert.rejects(
    callPageFunction(cdp.send, () => {}),
    /blocked/
  );
  assert.equal(cdp.calls.at(-1).method, 'Runtime.releaseObject');
});

test('transport errors survive cleanup failure', async () => {
  const cdp = transport(new Error('transport failed'), true);
  await assert.rejects(
    callPageFunction(cdp.send, () => {}),
    /transport failed/
  );
  assert.equal(cdp.calls.at(-1).method, 'Runtime.releaseObject');
});

test('successful return survives cleanup failure', async () => {
  const cdp = transport({ result: { value: false } }, true);
  assert.equal(await callPageFunction(cdp.send, () => {}), false);
});

test('rejects code strings before issuing any browser request', async () => {
  const cdp = transport();
  await assert.rejects(callPageFunction(cdp.send, 'alert(1)'), TypeError);
  assert.equal(cdp.calls.length, 0);
});

test('missing or exceptional page context fails safely', async () => {
  for (const response of [{ result: {} }, { exceptionDetails: { text: 'navigated' } }]) {
    const calls = [];
    await assert.rejects(
      callPageFunction(
        async (method) => {
          calls.push(method);
          return response;
        },
        () => {}
      ),
      /Page context unavailable/
    );
    assert.deepEqual(calls, ['Runtime.evaluate']);
  }
});
