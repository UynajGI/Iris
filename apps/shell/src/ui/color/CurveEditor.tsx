import { useRef, useState, type PointerEvent } from 'react';
import type { Adjustments } from '../../editing-client.js';

type Channel = 'master_curve' | 'red_curve' | 'green_curve' | 'blue_curve';
type Point = { x: number; y: number };
const channels: [Channel, string][] = [
  ['master_curve', 'RGB'],
  ['red_curve', '红'],
  ['green_curve', '绿'],
  ['blue_curve', '蓝'],
];
export function CurveEditor({
  adjustments,
  onChange,
}: {
  adjustments: Adjustments;
  onChange(value: Adjustments): void;
}) {
  const [channel, setChannel] = useState<Channel>('master_curve');
  const drag = useRef<number | null>(null);
  const saved = adjustments[channel].points;
  const points = saved.length
    ? saved
    : [
        { x: 0, y: 0 },
        { x: 1, y: 1 },
      ];
  const current = useRef(points);
  current.current = points;
  const save = (next: Point[]) => {
    current.current = next;
    onChange({ ...adjustments, [channel]: { points: next } });
  };
  const position = (event: PointerEvent<SVGSVGElement>) => {
    const bounds = event.currentTarget.getBoundingClientRect();
    return {
      x: Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width)),
      y: Math.max(0, Math.min(1, 1 - (event.clientY - bounds.top) / bounds.height)),
    };
  };
  const move = (index: number, position: Point) => {
    const next = current.current.map((point) => ({ ...point }));
    const low = next[index - 1],
      high = next[index + 1];
    next[index] = {
      x:
        index === 0
          ? 0
          : index === next.length - 1
            ? 1
            : Math.max(low!.x + 0.001, Math.min(high!.x - 0.001, position.x)),
      y: Math.max(low?.y ?? 0, Math.min(high?.y ?? 1, position.y)),
    };
    save(next);
  };
  return (
    <div className={`grade-curves curve-${channel}`}>
      <div className="grade-curve-channels segmented" role="group" aria-label="曲线通道">
        {channels.map(([key, label]) => (
          <button
            type="button"
            key={key}
            aria-pressed={channel === key}
            onClick={() => setChannel(key)}
          >
            {label}
          </button>
        ))}
      </div>
      <svg
        className="grade-curve-plot"
        viewBox="0 0 200 200"
        aria-label="曲线控制点"
        onPointerDown={(event) => {
          const p = position(event);
          let index = points.findIndex((point) => Math.hypot(point.x - p.x, point.y - p.y) < 0.06);
          if (index < 0 && points.length < 64 && p.x > 0.002 && p.x < 0.998) {
            const next = [...points, p].sort((left, right) => left.x - right.x);
            index = next.indexOf(p);
            p.y = Math.max(next[index - 1]!.y, Math.min(next[index + 1]!.y, p.y));
            if (next[index - 1]!.x + 0.001 < p.x && next[index + 1]!.x - 0.001 > p.x) save(next);
            else index = -1;
          }
          if (index >= 0) {
            drag.current = index;
            event.currentTarget.setPointerCapture(event.pointerId);
          }
        }}
        onPointerMove={(event) => {
          if (drag.current !== null) move(drag.current, position(event));
        }}
        onPointerUp={() => {
          drag.current = null;
        }}
        onPointerCancel={() => {
          drag.current = null;
        }}
      >
        {[0.25, 0.5, 0.75].map((value) => (
          <path
            key={value}
            className="grade-curve-grid"
            d={`M${value * 200} 0V200 M0 ${value * 200}H200`}
          />
        ))}
        <path className="grade-curve-identity" d="M0 200L200 0" />
        <polyline
          className="grade-curve-line"
          points={points.map((point) => `${point.x * 200},${(1 - point.y) * 200}`).join(' ')}
        />
        {points.map((point, index) => (
          <circle
            key={index}
            cx={point.x * 200}
            cy={(1 - point.y) * 200}
            r="4"
            tabIndex={0}
            role="button"
            aria-label={`控制点 ${index + 1}，输入 ${Math.round(point.x * 100)}，输出 ${Math.round(point.y * 100)}`}
            onKeyDown={(event) => {
              const key = event.key;
              if (key.startsWith('Arrow')) {
                event.preventDefault();
                move(index, {
                  x: point.x + (key === 'ArrowRight' ? 0.01 : key === 'ArrowLeft' ? -0.01 : 0),
                  y: point.y + (key === 'ArrowUp' ? 0.01 : key === 'ArrowDown' ? -0.01 : 0),
                });
              }
              if (
                (key === 'Delete' || key === 'Backspace') &&
                index > 0 &&
                index < points.length - 1
              ) {
                event.preventDefault();
                save(points.filter((_, i) => i !== index));
              }
            }}
          />
        ))}
      </svg>
    </div>
  );
}
