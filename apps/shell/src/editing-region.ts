import type { components } from './generated/api.js';

export type RegionView = components['schemas']['RegionView'];
export type NativeRegion = {
  blob: Blob;
  x: number;
  y: number;
  width: number;
  height: number;
  fullWidth: number;
  fullHeight: number;
};
export type ImageSize = { width: number; height: number };
export type RegionCenter = [number, number];

export function nativeImageOrigin(
  full: ImageSize,
  view: ImageSize,
  center: RegionCenter,
  ratio: number,
  region?: NativeRegion
) {
  // The server rounds normalized f32 coordinates; use its integer origin once
  // available so half-pixel ties cannot expose a row of the proxy underneath.
  const axis = (total: number, visible: number, middle: number, origin?: number) =>
    total >= visible && origin !== undefined
      ? -origin / ratio
      : -Math.round(middle * total - visible / 2) / ratio;
  return {
    left: axis(full.width, view.width, center[0], region?.x),
    top: axis(full.height, view.height, center[1], region?.y),
  };
}

export function decodeNativeRegion(
  response: { blob: Blob; headers: Headers },
  revision: number,
  sequence: number
): NativeRegion {
  const { headers, blob } = response;
  const number = (key: string) => {
    const raw = headers.get(`x-iris-${key}`);
    const value = raw === null ? NaN : Number(raw);
    if (!Number.isSafeInteger(value) || value < 0) throw new Error('原尺寸响应缺少有效坐标');
    return value;
  };
  if (
    number('revision') !== revision ||
    number('sequence') !== sequence ||
    headers.get('x-iris-approximate') !== 'false'
  )
    throw new Error('原尺寸响应已过期或不是完整像素');
  const result = {
    blob,
    x: number('region-x'),
    y: number('region-y'),
    width: number('region-width'),
    height: number('region-height'),
    fullWidth: number('full-width'),
    fullHeight: number('full-height'),
  };
  if (
    !result.width ||
    !result.height ||
    result.width > 2048 ||
    result.height > 2048 ||
    result.width * result.height > 4_000_000 ||
    result.x + result.width > result.fullWidth ||
    result.y + result.height > result.fullHeight
  )
    throw new Error('原尺寸响应的范围无效');
  return result;
}

/** One source pixel per physical display pixel, including high-DPI displays. */
export function nativeViewport(size: ImageSize, ratio: number, center: RegionCenter): RegionView {
  const width = Math.max(1, Math.ceil(size.width * ratio));
  const height = Math.max(1, Math.ceil(size.height * ratio));
  const scale = Math.min(1, 2048 / width, 2048 / height, Math.sqrt(4_000_000 / (width * height)));
  return {
    center,
    width: Math.max(1, Math.floor(width * scale)),
    height: Math.max(1, Math.floor(height * scale)),
  };
}

export function clampNativeCenter(
  center: RegionCenter,
  full: ImageSize,
  view: ImageSize
): RegionCenter {
  const axis = (c: number, total: number, visible: number) => {
    const half = Math.min(0.5, visible / (2 * total));
    return Math.max(half, Math.min(1 - half, c));
  };
  return [axis(center[0], full.width, view.width), axis(center[1], full.height, view.height)];
}
