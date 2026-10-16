export type Crop = [number, number, number, number];
export function dragCrop(base: Crop, handle: string, dx: number, dy: number): Crop {
  const clamp = (value: number, low: number, high: number) => Math.max(low, Math.min(high, value));
  if (handle === 'move')
    return [
      clamp(base[0] + dx, 0, 1 - base[2]),
      clamp(base[1] + dy, 0, 1 - base[3]),
      base[2],
      base[3],
    ];
  let left = base[0],
    top = base[1],
    right = left + base[2],
    bottom = top + base[3];
  if (handle.includes('w')) left = clamp(left + dx, 0, right - 0.01);
  if (handle.includes('e')) right = clamp(right + dx, left + 0.01, 1);
  if (handle.includes('n')) top = clamp(top + dy, 0, bottom - 0.01);
  if (handle.includes('s')) bottom = clamp(bottom + dy, top + 0.01, 1);
  return [left, top, right - left, bottom - top];
}
