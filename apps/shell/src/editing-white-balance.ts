import type { EditDocument, EditRecord, PreparedRawWhiteBalance } from './editing-client.js';

export function whiteBalanceKey(record: EditRecord, document: EditDocument): string {
  return JSON.stringify([
    record.id,
    record.revision,
    document.source.fingerprint,
    document.source.raw_white_balance ?? null,
  ]);
}

/** Apply only a suggestion belonging to the current source/revision. */
export function applyWhiteBalance(
  record: EditRecord,
  document: EditDocument,
  prepared: PreparedRawWhiteBalance,
  asShot = false
): EditDocument {
  if (
    prepared.revision !== record.revision ||
    prepared.source_fingerprint !== document.source.fingerprint
  )
    throw new Error('照片版本已变化，请重新调整白平衡');
  const gains = prepared.gains_rgb;
  if (
    gains.length !== 3 ||
    gains.some((value) => !Number.isFinite(value) || value < 0.01 || value > 100) ||
    Math.abs(gains[1]! - 1) >= 1e-5
  )
    throw new Error('白平衡参数不可用');
  return {
    ...document,
    source: { ...document.source, raw_white_balance: asShot ? null : [...gains] },
  };
}

/** Source-normalized point, before edit geometry; never sample the display RGB. */
export function neutralPatch(point: [number, number], source: EditDocument['source']) {
  if (
    ![...point, source.width, source.height].every(Number.isFinite) ||
    source.width < 1 ||
    source.height < 1 ||
    !Number.isInteger(source.width) ||
    !Number.isInteger(source.height)
  )
    throw new Error('原片尺寸不可用');
  const width = Math.min(8, source.width),
    height = Math.min(8, source.height);
  const bound = (value: number, size: number, edge: number) =>
    Math.max(
      0,
      Math.min(size - edge, Math.floor(Math.max(0, Math.min(1, value)) * size - edge / 2))
    );
  return {
    x: bound(point[0], source.width, width),
    y: bound(point[1], source.height, height),
    width,
    height,
  };
}
