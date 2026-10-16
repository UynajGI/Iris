import { useState } from 'react';

export function WhiteBalancePicker({
  width,
  height,
  onPick,
  onCancel,
}: {
  width: number;
  height: number;
  onPick(point: [number, number]): void;
  onCancel(): void;
}) {
  const [point, setPoint] = useState<[number, number]>([0.5, 0.5]);
  return (
    <svg
      className="grade-white-balance-picker"
      viewBox="0 0 1 1"
      preserveAspectRatio="none"
      role="button"
      tabIndex={0}
      aria-label="选择中性灰或白色区域；方向键移动，回车取样，Escape 取消"
      onPointerDown={(event) => {
        event.stopPropagation();
        event.preventDefault();
      }}
      onPointerMove={(event) => {
        const rect = event.currentTarget.getBoundingClientRect();
        setPoint([
          Math.max(0, Math.min(1, (event.clientX - rect.left) / rect.width)),
          Math.max(0, Math.min(1, (event.clientY - rect.top) / rect.height)),
        ]);
      }}
      onClick={(event) => {
        const rect = event.currentTarget.getBoundingClientRect();
        onPick([
          (event.clientX - rect.left) / rect.width,
          (event.clientY - rect.top) / rect.height,
        ]);
      }}
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          event.preventDefault();
          onCancel();
        } else if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault();
          onPick(point);
        } else if (['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) {
          event.preventDefault();
          const step = event.shiftKey ? 8 : 1;
          setPoint(([x, y]) => [
            Math.max(
              0,
              Math.min(
                1,
                x +
                  (event.key === 'ArrowLeft'
                    ? -step / width
                    : event.key === 'ArrowRight'
                      ? step / width
                      : 0)
              )
            ),
            Math.max(
              0,
              Math.min(
                1,
                y +
                  (event.key === 'ArrowUp'
                    ? -step / height
                    : event.key === 'ArrowDown'
                      ? step / height
                      : 0)
              )
            ),
          ]);
        }
      }}
    >
      <rect
        x={point[0] - 4 / width}
        y={point[1] - 4 / height}
        width={8 / width}
        height={8 / height}
      />
    </svg>
  );
}
