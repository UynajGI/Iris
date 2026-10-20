import { useEffect, useRef } from 'react';
import { type Channel, type ScopeData } from './scopes.js';
import { densityPeak, paintDensity, vectorDensity, waveformDensity } from './scope-raster.js';
import { scopeChroma, vectorPosition, vectorTargets } from './scope-chroma.js';

function drawTrace(
  canvas: HTMLCanvasElement,
  data: ScopeData,
  selected: Channel[],
  parade: boolean,
  vector: boolean
) {
  const context = canvas.getContext('2d');
  if (!context) return;
  const width = Math.max(1, Math.round(canvas.clientWidth));
  const height = Math.max(1, Math.round(canvas.clientHeight));
  canvas.width = width;
  canvas.height = height;
  const count = parade ? 3 : 1;
  const gap = parade ? 8 : 0;
  const panelWidth = Math.max(1, Math.floor((width - gap * (count - 1)) / count));
  const active: Channel[] = parade ? ['r', 'g', 'b'] : selected;
  const bins = vector
    ? [vectorDensity(data, width, height)]
    : active.map((channel) => waveformDensity(data, channel, panelWidth, height));
  const peak = densityPeak(bins);
  context.clearRect(0, 0, width, height);
  if (parade) {
    bins.forEach((density, i) => {
      const pixels = context.createImageData(panelWidth, height);
      paintDensity(pixels.data, density, active[i]!, peak);
      context.putImageData(pixels, i * (panelWidth + gap), 0);
    });
  } else {
    const pixels = context.createImageData(width, height);
    bins.forEach((density, i) => {
      const channel = vector ? 'l' : active[i]!;
      // RGB overlap may become neutral, but Y must not bleach the colour traces.
      const weight = bins.length > 1 ? (channel === 'l' ? 0.28 : 0.55) : 1;
      paintDensity(pixels.data, density, channel, peak, weight);
    });
    context.putImageData(pixels, 0, 0);
  }
}

export function ScopeTrace({
  data,
  selected,
  mode,
}: {
  data: ScopeData;
  selected: Channel[];
  mode: 'waveform' | 'parade' | 'vectorscope';
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const element = canvas.current;
    if (!element) return;
    const draw = () =>
      drawTrace(element, data, selected, mode === 'parade', mode === 'vectorscope');
    const observer = new ResizeObserver(draw);
    observer.observe(element);
    draw();
    return () => observer.disconnect();
  }, [data, selected, mode]);
  if (mode === 'vectorscope')
    return (
      <div className="scope-vector" aria-hidden="true">
        <canvas ref={canvas} />
        <VectorGraticule />
      </div>
    );
  return (
    <>
      <div className="scope-signal" aria-hidden="true">
        <canvas ref={canvas} />
        <div className="scope-signal-grid">
          {[0, 25, 50, 75, 100].map((level) => (
            <i key={level} style={{ top: `${level}%` }} />
          ))}
        </div>
      </div>
      <div className="waveform-scale" aria-hidden="true">
        {[100, 75, 50, 25, 0].map((level) => (
          <span key={level} style={{ top: `${100 - level}%` }}>
            {level}
          </span>
        ))}
      </div>
      {mode === 'parade' && (
        <div className="scope-parade-labels" aria-hidden="true">
          <span>R</span>
          <span>G</span>
          <span>B</span>
        </div>
      )}
    </>
  );
}

function VectorGraticule() {
  return (
    <svg viewBox="0 0 256 256" className="scope-vector-grid" aria-hidden="true">
      <g className="scope-graticule">
        <circle cx="128" cy="128" r="118" />
        <path d="M128 10V246M10 128H246M124 128h8M128 124v8" />
      </g>
      {vectorTargets.map(({ label, rgb, color }) => {
        const { cb, cr } = scopeChroma(rgb[0], rgb[1], rgb[2]);
        const outer = vectorPosition(cb, cr);
        const inner = vectorPosition(cb * 0.75, cr * 0.75);
        const labelPoint = vectorPosition(cb * 1.16, cr * 1.16);
        return (
          <g key={label} style={{ color }}>
            <rect
              className="scope-target outer"
              x={outer.x - 3}
              y={outer.y - 3}
              width="6"
              height="6"
            />
            <rect className="scope-target" x={inner.x - 4} y={inner.y - 4} width="8" height="8" />
            <text x={labelPoint.x} y={labelPoint.y} textAnchor="middle" dominantBaseline="central">
              {label}
            </text>
          </g>
        );
      })}
    </svg>
  );
}
