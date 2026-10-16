import { useRef, type PointerEvent } from 'react';
import { dragCrop, type Crop } from '../../editing-crop.js';

export function CropOverlay({
  crop,
  width,
  height,
  locked = false,
  onChange,
}: {
  crop: number[];
  width: number;
  height: number;
  locked?: boolean;
  onChange(crop: Crop): void;
}) {
  const gesture = useRef<{
    id: number;
    point: [number, number];
    base: Crop;
    handle: string;
  } | null>(null);
  const point = (event: PointerEvent<SVGSVGElement>): [number, number] => {
    const rect = event.currentTarget.getBoundingClientRect();
    return [(event.clientX - rect.left) / rect.width, (event.clientY - rect.top) / rect.height];
  };
  const move = (event: PointerEvent<SVGSVGElement>) => {
    const active = gesture.current;
    if (!active || active.id !== event.pointerId) return;
    const p = point(event);
    onChange(
      dragCrop(active.base, active.handle, p[0] - active.point[0], p[1] - active.point[1], locked)
    );
  };
  const [x, y, w, h] = crop as Crop;
  const handles = [
    ['nw', x, y],
    ['n', x + w / 2, y],
    ['ne', x + w, y],
    ['w', x, y + h / 2],
    ['e', x + w, y + h / 2],
    ['sw', x, y + h],
    ['s', x + w / 2, y + h],
    ['se', x + w, y + h],
  ] as const;
  return (
    <svg
      className="grade-crop-overlay"
      viewBox="0 0 1 1"
      preserveAspectRatio="none"
      aria-label="画布裁切边框"
      tabIndex={0}
      onKeyDown={(event) => {
        if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return;
        event.preventDefault();
        const dx = event.key === 'ArrowLeft' ? -0.005 : event.key === 'ArrowRight' ? 0.005 : 0;
        const dy = event.key === 'ArrowUp' ? -0.005 : event.key === 'ArrowDown' ? 0.005 : 0;
        onChange(dragCrop([x, y, w, h], event.shiftKey ? 'se' : 'move', dx, dy, locked));
      }}
      onPointerDown={(event) => {
        const handle = (event.target as Element).getAttribute('data-handle');
        if (!handle) return;
        gesture.current = { id: event.pointerId, point: point(event), base: [x, y, w, h], handle };
        event.currentTarget.setPointerCapture(event.pointerId);
        event.stopPropagation();
      }}
      onPointerMove={move}
      onPointerUp={(event) => {
        move(event);
        gesture.current = null;
      }}
      onPointerCancel={() => {
        gesture.current = null;
      }}
    >
      <path
        d={`M0,0H1V1H0Z M${x},${y}h${w}v${h}h${-w}Z`}
        fillRule="evenodd"
        fill="black"
        opacity="0.55"
      />
      <rect x={x} y={y} width={w} height={h} data-handle="move" className="grade-crop-frame" />
      {[1, 2].map((i) => (
        <g key={i} className="grade-crop-thirds">
          <line x1={x + (w * i) / 3} x2={x + (w * i) / 3} y1={y} y2={y + h} />
          <line x1={x} x2={x + w} y1={y + (h * i) / 3} y2={y + (h * i) / 3} />
        </g>
      ))}
      {handles.map(([handle, cx, cy]) => (
        <rect
          key={handle}
          data-handle={handle}
          className={`grade-crop-handle crop-${handle}`}
          x={cx - 5 / width}
          y={cy - 5 / height}
          width={10 / width}
          height={10 / height}
        />
      ))}
    </svg>
  );
}
