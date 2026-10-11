// Only pass trusted function literals here; JSON values travel separately as CDP arguments.
export async function callPageFunction(send, callback, ...values) {
  const functionDeclaration = Function.prototype.toString.call(callback);
  const target = await send('Runtime.evaluate', {
    expression: 'globalThis',
    returnByValue: false,
  });
  const objectId = target.result?.objectId;
  if (target.exceptionDetails || !objectId) throw new Error('Page context unavailable');
  try {
    const response = await send('Runtime.callFunctionOn', {
      objectId,
      functionDeclaration,
      arguments: values.map((value) => ({ value })),
      awaitPromise: true,
      returnByValue: true,
      userGesture: true,
    });
    if (response.exceptionDetails)
      throw new Error(
        response.exceptionDetails.exception?.description ?? response.exceptionDetails.text
      );
    return response.result.value;
  } finally {
    // Best-effort cleanup must not hide the original browser/transport error.
    await send('Runtime.releaseObject', { objectId }).catch(() => {});
  }
}
