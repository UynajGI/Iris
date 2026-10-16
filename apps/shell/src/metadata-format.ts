/** EXIF readouts retain camera-local time; they never infer timezone or camera settings. */
export function cameraName(make?: string | null, model?: string | null): string {
  if (!make) return model || '—';
  if (!model) return make;
  return model.toLowerCase() === make.toLowerCase() ||
    model.toLowerCase().startsWith(`${make.toLowerCase()} `)
    ? model
    : `${make} ${model}`;
}
export function decimal(value: number | null | undefined, digits = 2): string {
  return value == null || !Number.isFinite(value) ? '—' : String(Number(value.toFixed(digits)));
}
export function shutter(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds) || seconds <= 0) return '—';
  if (seconds >= 1) return `${decimal(seconds, 3)} s`;
  const reciprocal = 1 / seconds;
  const rounded = Math.round(reciprocal);
  return Math.abs(reciprocal - rounded) / reciprocal < 0.005
    ? `1/${rounded} s`
    : `${decimal(seconds, 4)} s`;
}
export function captureDate(date: string | null | undefined, offset?: string | null): string {
  if (!date) return '—';
  const readable = date.replace(/^(\d{4}):(\d{2}):(\d{2}) /, '$1-$2-$3 ');
  return offset && /^[+-]\d{2}:\d{2}$/.test(offset) ? `${readable} ${offset}` : readable;
}
export function ev(value: number | null | undefined): string {
  return value == null || !Number.isFinite(value)
    ? '—'
    : `${value > 0 ? '+' : ''}${decimal(value)} EV`;
}
export function tagLabel(value: number | null | undefined, names: Record<number, string>): string {
  if (value == null) return '—';
  return names[value] ?? `未知（${value}）`;
}
export const exposurePrograms: Record<number, string> = {
  0: '未定义',
  1: '手动',
  2: '程序自动',
  3: '光圈优先',
  4: '快门优先',
  5: '创意程序',
  6: '运动程序',
  7: '人像',
  8: '风景',
};
export const meteringModes: Record<number, string> = {
  0: '未知',
  1: '平均',
  2: '中央重点',
  3: '点测光',
  4: '多点',
  5: '评价 / 矩阵',
  6: '局部',
  255: '其他',
};
export function flashLabel(value: number | null | undefined): string {
  if (value == null) return '—';
  if (value & 0x20) return '无闪光灯功能';
  return value & 1 ? '已闪光' : '未闪光';
}
