export interface CompareView {
  scale: number;
  x: number;
  y: number;
}
export const fitCompareView: CompareView = { scale: 1, x: 0, y: 0 };
export function boundedCompareView(view: CompareView): CompareView {
  const scale = Number.isFinite(view.scale) ? Math.max(1, Math.min(6, view.scale)) : 1;
  const bound = (scale - 1) / 2;
  if (bound === 0) return fitCompareView;
  const clamp = (n: number) => (Number.isFinite(n) ? Math.max(-bound, Math.min(bound, n)) : 0);
  return { scale, x: clamp(view.x), y: clamp(view.y) };
}
export function zoomCompareView(view: CompareView, factor: number): CompareView {
  return boundedCompareView({ scale: view.scale * factor, x: view.x * factor, y: view.y * factor });
}
