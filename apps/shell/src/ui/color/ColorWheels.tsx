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
  const [band, setBand] = useState<Band | 'all'>('all');
  // Neutral RGB has no hue. Retain the chosen angle until saturation is raised.
  const [hues, setHues] = useState({ shadows: 0, midtones: 0, highlights: 0 });
  const valueFor = (key: Band) => {
    const actual = gainsToWheel(adjustments.color_balance[key]);
    return { ...actual, hue: actual.saturation < 1e-6 ? hues[key] : actual.hue };
  };
  const change = (key: Band, patch: Partial<WheelColor>) => {
    if (disabled) return;
    const next = { ...valueFor(key), ...patch };
    setHues((previous) => ({ ...previous, [key]: ((next.hue % 360) + 360) % 360 }));
    const gains = wheelToGains(next);
    if (
      gains.every((gain, index) => Math.abs(gain - adjustments.color_balance[key][index]!) < 1e-9)
    )
      return;
    onChange({ ...adjustments, color_balance: { ...adjustments.color_balance, [key]: gains } });
  };
  const value = valueFor(band === 'all' ? 'midtones' : band);
  return (
    <>
      <div className="grade-wheel-bands" role="group" aria-label="颜色分级明暗范围">
        <button type="button" aria-pressed={band === 'all'} onClick={() => setBand('all')}>
          三轮
        </button>
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
      {band === 'all' ? (
        <div className="grade-wheels-three">
          {(['midtones', 'shadows', 'highlights'] as const).map((key) => {
            const name = bands.find(([item]) => item === key)![1];
            return (
              <div className="grade-wheel-mini" key={key}>
                <button type="button" onClick={() => setBand(key)} title={`放大${name}色轮`}>
                  {name}
                </button>
                <WheelDial
                  label={name}
                  value={valueFor(key)}
                  disabled={disabled}
                  onChange={(patch) => change(key, patch)}
                />
                <AdjustmentSlider
                  spec={{
                    key: `${key}-luminance`,
                    label: `${name}亮度`,
                    min: -2,
                    max: 2,
                    scale: 50,
                  }}
                  value={valueFor(key).luminance}
                  onChange={(luminance) => change(key, { luminance })}
                />
              </div>
            );
          })}
        </div>
      ) : (
        <>
          <WheelDial
            key={band}
            label={bands.find(([key]) => key === band)![1]}
            value={value}
            disabled={disabled}
            onChange={(patch) => change(band, patch)}
          />
          <div className="grade-wheel-values">
            <span>H {Math.round(value.hue)}°</span>
            <span>S {Math.round(value.saturation * 50)}</span>
          </div>
          <details className="grade-wheel-controls">
            <summary>色相与饱和度</summary>
            <AdjustmentSlider
              spec={{ key: 'wheel-hue', label: '色相', min: 0, max: 359, step: 1, unit: '°' }}
              value={value.hue}
              onChange={(hue) => change(band, { hue })}
            />
            <AdjustmentSlider
              spec={{ key: 'wheel-saturation', label: '饱和度', min: 0, max: 2, scale: 50 }}
              value={value.saturation}
              onChange={(saturation) => change(band, { saturation })}
            />
          </details>
          <AdjustmentSlider
            spec={{ key: 'wheel-luminance', label: '亮度', min: -2, max: 2, scale: 50 }}
            value={value.luminance}
            onChange={(luminance) => change(band, { luminance })}
          />
        </>
      )}
    </>
  );
}
