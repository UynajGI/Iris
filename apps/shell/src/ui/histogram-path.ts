// SPDX-License-Identifier: GPL-3.0-or-later

const outlineKernel = [1, 8, 28, 56, 70, 56, 28, 8, 1];

/** Display-only smoothing over four bins either side; retain measured clipping spikes. */
export function histogramOutline(bins: Uint32Array, peak: number): number[] {
  return Array.from(bins, (count, i) => {
    const value =
      i === 0 || i === bins.length - 1
        ? count
        : outlineKernel.reduce(
            (sum, weight, k) =>
              sum + weight * bins[Math.max(0, Math.min(bins.length - 1, i + k - 4))]!,
            0
          ) / 256;
    return 100 - Math.min(1, value / Math.max(1, peak)) * 100;
  });
}

/** Quadratic joins stay inside their neighbouring samples' convex hull. */
export function histogramPath(bins: Uint32Array, peak: number): string {
  const heights = histogramOutline(bins, peak);
  if (!heights.length) return '';
  const last = heights.length - 1;
  let path = `M0 100 L0 ${heights[0]}`;
  for (let i = 1; i < last; i++)
    path += ` Q${i - 1} ${heights[i - 1]} ${i - 0.5} ${(heights[i - 1]! + heights[i]!) / 2}`;
  if (last > 0) path += ` Q${last - 1} ${heights[last - 1]} ${last} ${heights[last]}`;
  return `${path} L${last} 100Z`;
}
