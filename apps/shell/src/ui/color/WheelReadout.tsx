import { useRef, useState } from 'react';
import type { WheelColor } from '../../editing-color-wheel.js';

/** Same user-facing scale as the numeric controls; no degree sign or changing widths. */
export function WheelReadout({
  value,
  label,
  disabled,
  onChange,
}: {
  value: WheelColor;
  label: string;
  disabled: boolean;
  onChange(patch: Partial<WheelColor>): void;
}) {
  return (
    <div className="grade-wheel-readout" role="group" aria-label={`${label} HSL 数值`}>
      <WheelNumber
        name="H"
        label={label}
        value={Math.round(value.hue) % 360}
        min={0}
        max={359}
        disabled={disabled}
        onChange={(hue) => onChange({ hue })}
      />
      <WheelNumber
        name="S"
        label={label}
        value={Math.round(value.saturation * 50)}
        min={0}
        max={100}
        disabled={disabled}
        onChange={(saturation) => onChange({ saturation: saturation / 50 })}
      />
      <WheelNumber
        name="L"
        label={label}
        value={Math.round(value.luminance * 50)}
        min={-100}
        max={100}
        disabled={disabled}
        onChange={(luminance) => onChange({ luminance: luminance / 50 })}
      />
    </div>
  );
}

function WheelNumber({
  name,
  label,
  value,
  min,
  max,
  disabled,
  onChange,
}: {
  name: string;
  label: string;
  value: number;
  min: number;
  max: number;
  disabled: boolean;
  onChange(value: number): void;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  const original = useRef(value);
  return (
    <span className="grade-wheel-number">
      <span>{name}:</span>
      {draft === null ? (
        <button
          type="button"
          disabled={disabled}
          aria-label={`${label} ${name}，点击输入`}
          style={{ width: name === 'L' ? '4ch' : '3ch' }}
          onClick={() => {
            original.current = value;
            setDraft(String(value));
          }}
        >
          {value}
        </button>
      ) : (
        <input
          aria-label={`${label} ${name}`}
          type="number"
          min={min}
          max={max}
          step={1}
          disabled={disabled}
          value={draft}
          autoFocus
          style={{ width: name === 'L' ? '4ch' : '3ch' }}
          onFocus={(event) => {
            event.currentTarget.select();
          }}
          onChange={(event) => {
            const text = event.target.value;
            setDraft(text);
            if (text.trim() && Number.isFinite(Number(text)))
              onChange(Math.max(min, Math.min(max, Math.round(Number(text)))));
          }}
          onBlur={() => setDraft(null)}
          onKeyDown={(event) => {
            event.stopPropagation();
            if (event.key === 'Enter') {
              event.preventDefault();
              event.currentTarget.blur();
            }
            if (event.key === 'Escape') {
              event.preventDefault();
              onChange(original.current);
              event.currentTarget.blur();
            }
          }}
        />
      )}
    </span>
  );
}
