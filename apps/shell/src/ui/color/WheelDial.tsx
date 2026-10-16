import type { KeyboardEvent, PointerEvent } from 'react';
import { wheelPoint, type WheelColor } from '../../editing-color-wheel.js';

export function WheelDial({
  label,
  value,
  disabled,
  onChange,
}: {
  label: string;
  value: WheelColor;
  disabled: boolean;
  onChange(patch: Partial<WheelColor>): void;
}) {
  const drag = (event: PointerEvent<HTMLDivElement>, ring: boolean) => {
    if (disabled) return;
    const box = event.currentTarget.getBoundingClientRect();
    const point = wheelPoint(
      (event.clientX - box.left) / box.width,
      (event.clientY - box.top) / box.height,
      value.hue
    );
    onChange(ring ? { hue: point.hue } : point);
  };
  const keys = (event: KeyboardEvent<HTMLDivElement>, ring: boolean) => {
    if (disabled) return;
    if (!['Home', 'End', 'ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key))
      return;
    event.preventDefault();
    event.stopPropagation();
    const positive = ['ArrowRight', 'ArrowUp'].includes(event.key);
    const step = (event.shiftKey ? 10 : 1) * (positive ? 1 : -1);
    if (ring)
      onChange({
        hue: event.key === 'Home' ? 0 : event.key === 'End' ? 359 : (value.hue + step + 360) % 360,
      });
    else
      onChange({
        saturation:
          event.key === 'Home'
            ? 0
            : event.key === 'End'
              ? 2
              : Math.max(0, Math.min(2, value.saturation + step / 50)),
      });
  };
  const gesture = (ring: boolean) => ({
    onPointerDown: (event: PointerEvent<HTMLDivElement>) => {
      if (disabled || event.button !== 0) return;
      event.preventDefault();
      event.currentTarget.focus();
      event.currentTarget.setPointerCapture(event.pointerId);
      drag(event, ring);
    },
    onPointerMove: (event: PointerEvent<HTMLDivElement>) => {
      if (event.currentTarget.hasPointerCapture(event.pointerId)) drag(event, ring);
    },
    onPointerUp: (event: PointerEvent<HTMLDivElement>) => {
      if (!event.currentTarget.hasPointerCapture(event.pointerId)) return;
      drag(event, ring);
      event.currentTarget.releasePointerCapture(event.pointerId);
    },
    onKeyDown: (event: KeyboardEvent<HTMLDivElement>) => keys(event, ring),
  });
  const angle = (value.hue * Math.PI) / 180;
  return (
    <div className="grade-color-wheels" role="group" aria-label={`${label}色轮`}>
      <div className="grade-wheel-dial">
        <div
          className="grade-hue-ring"
          role="slider"
          aria-label={`${label}色相环`}
          aria-valuemin={0}
          aria-valuemax={359}
          aria-valuenow={Math.round(value.hue) % 360}
          aria-valuetext={`${Math.round(value.hue)} 度`}
          aria-disabled={disabled}
          tabIndex={disabled ? -1 : 0}
          title="沿外环调整色相"
          {...gesture(true)}
        >
          <span
            className="grade-hue-marker"
            style={{ transform: `rotate(${value.hue}deg)` }}
            aria-hidden="true"
          />
        </div>
        <div
          className="grade-color-wheel"
          role="slider"
          aria-label={`${label}饱和度盘`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round(value.saturation * 50)}
          aria-valuetext={`色相 ${Math.round(value.hue)} 度，饱和度 ${Math.round(value.saturation * 50)}`}
          aria-disabled={disabled}
          tabIndex={disabled ? -1 : 0}
          title="从中心向边缘增加饱和度，双击归零"
          {...gesture(false)}
          onDoubleClick={() => {
            if (!disabled) onChange({ saturation: 0 });
          }}
        >
          <span
            className="grade-wheel-radius"
            aria-hidden="true"
            style={{ width: `${value.saturation * 23}%`, transform: `rotate(${value.hue}deg)` }}
          />
          <span className="grade-wheel-center" aria-hidden="true" />
          <span
            className="grade-wheel-puck"
            aria-hidden="true"
            style={{
              left: `${50 + Math.cos(angle) * value.saturation * 23}%`,
              top: `${50 + Math.sin(angle) * value.saturation * 23}%`,
            }}
          />
        </div>
      </div>
    </div>
  );
}
