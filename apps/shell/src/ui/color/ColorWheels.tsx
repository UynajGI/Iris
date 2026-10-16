import { useState } from 'react';
import type { Adjustments } from '../../editing-client.js';
import { gainsToWheel, wheelToGains, type WheelColor } from '../../editing-color-wheel.js';
import { AdjustmentSlider } from './AdjustmentPanels.js';
import { WheelDial } from './WheelDial.js';

const bands = [
  ['shadows', '阴影'],
  ['midtones', '中间调'],
  ['highlights', '高光'],
] as const;
type Band = (typeof bands)[number][0];
export function ColorWheels({
  adjustments,
  onChange,
  disabled,
}: {
  adjustments: Adjustments;
  onChange(value: Adjustments): void;
  disabled: boolean;
}) {
  const [band, setBand] = useState<Band>('midtones');
  // Neutral RGB has no hue. Retain the chosen angle until saturation is raised.
  const [hues, setHues] = useState({ shadows: 0, midtones: 0, highlights: 0 });
  const actual = gainsToWheel(adjustments.color_balance[band]);
  const value = { ...actual, hue: actual.saturation < 1e-6 ? hues[band] : actual.hue };
  const change = (patch: Partial<WheelColor>) => {
    if (disabled) return;
    const next = { ...value, ...patch };
    setHues((previous) => ({ ...previous, [band]: ((next.hue % 360) + 360) % 360 }));
    const gains = wheelToGains(next);
    if (
      gains.every((gain, index) => Math.abs(gain - adjustments.color_balance[band][index]!) < 1e-9)
    )
      return;
    onChange({ ...adjustments, color_balance: { ...adjustments.color_balance, [band]: gains } });
  };
  return (
    <>
      <div className="grade-wheel-bands" role="group" aria-label="颜色分级明暗范围">
        {bands.map(([key, name]) => (
          <button
            type="button"
            key={key}
            aria-pressed={key === band}
            disabled={disabled}
            onClick={() => setBand(key)}
          >
            {name}
          </button>
        ))}
      </div>
      <WheelDial
        label={bands.find(([key]) => key === band)![1]}
        value={value}
        disabled={disabled}
        onChange={change}
      />
      <AdjustmentSlider
        spec={{ key: 'wheel-hue', label: '色相', min: 0, max: 359, step: 1, unit: '°' }}
        value={value.hue}
        onChange={(hue) => change({ hue })}
      />
      <AdjustmentSlider
        spec={{ key: 'wheel-saturation', label: '饱和度', min: 0, max: 2, scale: 50 }}
        value={value.saturation}
        onChange={(saturation) => change({ saturation })}
      />
      <AdjustmentSlider
        spec={{ key: 'wheel-luminance', label: '亮度', min: -2, max: 2, scale: 50 }}
        value={value.luminance}
        onChange={(luminance) => change({ luminance })}
      />
    </>
  );
}
