import { useRef, type ReactNode } from 'react';
import { boundedCompareView, type CompareView } from '../compare-view.js';

export interface CompareViewportControl {
  view: CompareView;
  onChange(view: CompareView): void;
  onFocus?(): void;
}

/** Coordinates are fractions of the viewport, so linked panes may differ in size. */
export function CompareViewport({
  view,
  onChange,
  children,
  label,
  onFocus,
}: CompareViewportControl & { children: ReactNode; label: string }) {
  const drag = useRef<{
    id: number;
    x: number;
    y: number;
    view: CompareView;
    moved: boolean;
  } | null>(null);
  return (
    <div
      className="compare-viewport"
      tabIndex={0}
      role="group"
      aria-label={label}
      onPointerDown={(event) => {
        if (event.button !== 0) return;
        event.currentTarget.focus();
        onFocus?.();
        drag.current = {
          id: event.pointerId,
          x: event.clientX,
          y: event.clientY,
          view,
          moved: false,
        };
        event.currentTarget.setPointerCapture(event.pointerId);
      }}
      onPointerMove={(event) => {
        const start = drag.current;
        if (!start || start.id !== event.pointerId || start.view.scale === 1) return;
        const rect = event.currentTarget.getBoundingClientRect();
        start.moved ||= Math.hypot(event.clientX - start.x, event.clientY - start.y) > 3;
        onChange(
          boundedCompareView({
            ...start.view,
            x: start.view.x + (event.clientX - start.x) / Math.max(1, rect.width),
            y: start.view.y + (event.clientY - start.y) / Math.max(1, rect.height),
          })
        );
      }}
      onPointerUp={() => {
        drag.current = null;
      }}
      onPointerCancel={() => {
        drag.current = null;
      }}
      onKeyDown={(event) => {
        if (
          !event.altKey ||
          !['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)
        )
          return;
        event.preventDefault();
        event.stopPropagation();
        onChange(
          boundedCompareView({
            ...view,
            x: view.x + (event.key === 'ArrowLeft' ? 0.05 : event.key === 'ArrowRight' ? -0.05 : 0),
            y: view.y + (event.key === 'ArrowUp' ? 0.05 : event.key === 'ArrowDown' ? -0.05 : 0),
          })
        );
      }}
    >
      <div
        className="compare-viewport-content"
        style={{ transform: `translate(${view.x * 100}%, ${view.y * 100}%) scale(${view.scale})` }}
      >
        {children}
      </div>
    </div>
  );
}
