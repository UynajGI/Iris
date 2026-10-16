import type { KeyboardEvent } from 'react';
import type { WheelColor } from '../../editing-color-wheel.js';
import { useWheelGesture } from './useWheelGesture.js';

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
  const gesture = useWheelGesture(value, disabled, onChange);
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
          onKeyDown={(event) => keys(event, true)}
          onDoubleClick={() => {
            if (!disabled) onChange({ hue: 0 });
          }}
        >
          <span
            className="grade-hue-marker"
            style={{ transform: `rotate(${-value.hue}deg)` }}
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
          title="Shift：锁定色相；Ctrl / ⌘：锁定饱和度；Alt：精细调整；双击归零"
          {...gesture(false)}
          onKeyDown={(event) => keys(event, event.ctrlKey || event.metaKey)}
          onDoubleClick={() => {
            if (!disabled) onChange({ hue: 0, saturation: 0 });
          }}
        >
          <span
            className="grade-wheel-radius"
            aria-hidden="true"
            style={{ transform: `rotate(${-value.hue}deg)` }}
          />
          <span className="grade-wheel-center" aria-hidden="true" />
          <span
            className="grade-wheel-puck"
            aria-hidden="true"
            style={{
              left: `${50 + Math.cos(angle) * value.saturation * 23}%`,
              top: `${50 - Math.sin(angle) * value.saturation * 23}%`,
            }}
          />
        </div>
      </div>
    </div>
  );
}
