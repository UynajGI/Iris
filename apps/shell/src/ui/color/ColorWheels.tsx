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
function WheelControls({
  label,
  value,
  onChange,
}: {
  label: string;
  value: WheelColor;
  onChange(patch: Partial<WheelColor>): void;
}) {
  return (
    <div className="grade-wheel-controls" role="group" aria-label={`${label}数值调整`}>
      <AdjustmentSlider
        spec={{ key: 'wheel-hue', label: '色相', min: 0, max: 359, step: 1 }}
        value={value.hue}
        onChange={(hue) => onChange({ hue })}
      />
      <AdjustmentSlider
        spec={{ key: 'wheel-saturation', label: '饱和度', min: 0, max: 2, scale: 50 }}
        value={value.saturation}
        onChange={(saturation) => onChange({ saturation })}
      />
      <AdjustmentSlider
        spec={{ key: 'wheel-luminance', label: '亮度', min: -2, max: 2, scale: 50 }}
        value={value.luminance}
        onChange={(luminance) => onChange({ luminance })}
      />
    </div>
  );
}
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
  const [activeBand, setActiveBand] = useState<Band>('midtones');
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
  const selectedBand = band === 'all' ? activeBand : band;
  const selectedName = bands.find(([key]) => key === selectedBand)![1];
  const value = valueFor(selectedBand);
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
              <div
                className="grade-wheel-mini"
                key={key}
                onPointerDownCapture={() => setActiveBand(key)}
                onFocusCapture={() => setActiveBand(key)}
              >
                <button
                  type="button"
                  aria-pressed={activeBand === key}
                  onClick={() => setActiveBand(key)}
                  title={`调整${name}`}
                  disabled={disabled}
                >
                  {name}
                </button>
                <WheelDial
                  label={name}
                  value={valueFor(key)}
                  disabled={disabled}
                  onChange={(patch) => change(key, patch)}
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
        </>
      )}
      {band === 'all' && <div className="grade-wheel-control-label">{selectedName}</div>}
      <WheelControls
        label={selectedName}
        value={value}
        onChange={(patch) => change(selectedBand, patch)}
      />
    </>
  );
}
