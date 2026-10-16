import { useRef, type PointerEvent } from 'react';
import { WheelGesture } from '../../editing-wheel-gesture.js';
import type { WheelColor } from '../../editing-color-wheel.js';

export function useWheelGesture(
  value: WheelColor,
  disabled: boolean,
  onChange: (value: Partial<WheelColor>) => void
) {
  const active = useRef<{ id: number; gesture: WheelGesture } | null>(null);
  const point = (event: PointerEvent<HTMLDivElement>) => {
    const box = event.currentTarget.getBoundingClientRect();
    return { x: (event.clientX - box.left) / box.width, y: (event.clientY - box.top) / box.height };
  };
  const move = (event: PointerEvent<HTMLDivElement>) => {
    if (disabled || active.current?.id !== event.pointerId) return;
    onChange(active.current.gesture.move(point(event), event));
  };
  return (ring: boolean) => ({
    onPointerDown: (event: PointerEvent<HTMLDivElement>) => {
      if (disabled || event.button !== 0) return;
      event.preventDefault();
      event.currentTarget.focus();
      event.currentTarget.setPointerCapture(event.pointerId);
      const handle = (event.target as HTMLElement).classList.contains('grade-wheel-puck');
      active.current = {
        id: event.pointerId,
        gesture: new WheelGesture(value, point(event), event, ring, handle),
      };
      if (!handle && !event.altKey) move(event);
    },
    onPointerMove: move,
    onPointerUp: (event: PointerEvent<HTMLDivElement>) => {
      if (active.current?.id !== event.pointerId) return;
      move(event);
      active.current = null;
      event.currentTarget.releasePointerCapture(event.pointerId);
    },
    onPointerCancel: () => {
      active.current = null;
    },
    onLostPointerCapture: () => {
      active.current = null;
    },
  });
}
