import { useEffect, useRef, useState } from 'react';
import { clippingPixels, readScopePixels, type Clipping } from '../scope-pixels.js';

export function ClippingOverlay({ url, clipping }: { url: string; clipping: Clipping }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [painted, setPainted] = useState('');
  const key = `${clipping.shadows}:${clipping.highlights}`;
  useEffect(() => {
    let active = true;
    void readScopePixels(url)
      .then((pixels) => {
        const node = canvas.current;
        if (!active || !node) return;
        node.width = pixels.width;
        node.height = pixels.height;
        const context = node.getContext('2d');
        if (!context) return;
        context.putImageData(
          new ImageData(clippingPixels(pixels.data, clipping), pixels.width, pixels.height),
          0,
          0
        );
        setPainted(key);
      })
      .catch(() => {
        if (active) setPainted('');
      });
    return () => {
      active = false;
    };
  }, [url, key, clipping.shadows, clipping.highlights]);
  return (
    <canvas
      ref={canvas}
      className="grade-clipping-overlay"
      aria-label="预览裁切提示：蓝色暗部、红色高光"
      style={{ visibility: painted === key ? 'visible' : 'hidden' }}
    />
  );
}
