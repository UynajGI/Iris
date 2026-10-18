import { useId } from 'react';

export type SliderSpec = {
  key: string;
  label: string;
  min: number;
  max: number;
  step?: number;
  scale?: number;
  unit?: string;
  reset?: number;
  tone?: string;
};

/** Traditional panel row: name, track and an editable value; the track is clamped to the spec. */
export function AdjustmentSlider({
  spec,
  value,
  onChange,
}: {
  spec: SliderSpec;
  value: number;
  onChange(value: number): void;
}) {
  const id = useId();
  const scale = spec.scale ?? 1;
  const change = (number: number) => {
    if (Number.isFinite(number)) onChange(Math.max(spec.min, Math.min(spec.max, number)));
  };
  return (
    <div className="grade-slider">
      <label htmlFor={id} onDoubleClick={() => change(spec.reset ?? 0)} title="双击重置">
        {spec.label}
      </label>
      <input
        id={id}
        className={spec.tone ? `grade-track-${spec.tone}` : ''}
        type="range"
        min={spec.min}
        max={spec.max}
        step={spec.step ?? 0.01}
        value={value}
        aria-valuetext={`${Number((value * scale).toFixed(2))}${spec.unit ? ` ${spec.unit}` : ''}`}
        onChange={(event) => change(Number(event.target.value))}
      />
      <div className="grade-value">
        <input
          aria-label={`${spec.label}数值`}
          type="number"
          value={Number((value * scale).toFixed(scale === 1 ? 2 : 0))}
          min={spec.min * scale}
          max={spec.max * scale}
          step={(spec.step ?? 0.01) * scale}
          onChange={(event) => change(Number(event.target.value) / scale)}
        />
        {spec.unit && <span>{spec.unit}</span>}
      </div>
    </div>
  );
}
