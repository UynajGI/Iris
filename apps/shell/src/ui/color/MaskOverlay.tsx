import { useLayoutEffect, useRef } from 'react';
import type { MaskDefinition } from '../../editing-masks.js';
import { spatialMaskCoverage } from '../../editing-mask-overlay.js';

export function MaskOverlay({
  definition,
  width,
  height,
}: {
  definition: MaskDefinition;
  width: number;
  height: number;
}) {
  const ref = useRef<HTMLCanvasElement>(null);
  useLayoutEffect(() => {
    const canvas = ref.current,
      coverage = spatialMaskCoverage(definition, width, height);
    if (!canvas) return;
    const context = canvas.getContext('2d');
    if (!context) return;
    const scale = Math.min(1, 256 / Math.max(width, height));
    canvas.width = Math.max(1, Math.round(width * scale));
    canvas.height = Math.max(1, Math.round(height * scale));
    if (!coverage) {
      context.clearRect(0, 0, canvas.width, canvas.height);
      return;
    }
    const image = context.createImageData(canvas.width, canvas.height);
    for (let y = 0; y < canvas.height; y++)
      for (let x = 0; x < canvas.width; x++) {
        const i = (y * canvas.width + x) * 4;
        image.data[i] = 255;
        image.data[i + 1] = 35;
        image.data[i + 2] = 40;
        image.data[i + 3] = Math.round(
          coverage((x + 0.5) / canvas.width, (y + 0.5) / canvas.height) * 140
        );
      }
    context.putImageData(image, 0, 0);
  }, [definition, width, height]);
  return <canvas ref={ref} className="grade-mask-overlay" aria-label="红色蒙版覆盖" />;
}
