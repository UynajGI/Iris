import { useState, type PointerEvent } from 'react';
import type { Adjustments } from '../../editing-client.js';
import { gainsToWheel, wheelToGains, type WheelColor } from '../../editing-color-wheel.js';
import { AdjustmentSlider } from './AdjustmentPanels.js';

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
  const value = gainsToWheel(adjustments.color_balance[band]);
  const change = (key: Band, patch: Partial<WheelColor>) => {
    if (disabled) return;
    onChange({
      ...adjustments,
      color_balance: {
        ...adjustments.color_balance,
        [key]: wheelToGains({ ...gainsToWheel(adjustments.color_balance[key]), ...patch }),
      },
    });
  };
  const drag = (key: Band, event: PointerEvent<HTMLDivElement>) => {
    const box = event.currentTarget.getBoundingClientRect();
    const x = (event.clientX - box.left - box.width / 2) / (box.width * 0.45);
    const y = (event.clientY - box.top - box.height / 2) / (box.height * 0.45);
    change(key, {
      hue: ((Math.atan2(y, x) * 180) / Math.PI + 360) % 360,
      saturation: Math.min(1, Math.hypot(x, y)) * 2,
    });
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
      <div className="grade-color-wheels">
        {bands
          .filter(([key]) => key === band)
          .map(([key, name]) => {
            const color = gainsToWheel(adjustments.color_balance[key]);
            const angle = (color.hue * Math.PI) / 180;
            return (
              <div className="grade-wheel-group" key={key}>
                <div
                  className={`grade-color-wheel ${key === band ? 'selected' : ''}`}
                  role="slider"
                  tabIndex={disabled ? -1 : 0}
                  aria-disabled={disabled}
                  aria-label={`${name}色轮`}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-valuenow={Math.round(color.saturation * 50)}
                  aria-valuetext={`色相 ${Math.round(color.hue)} 度，饱和度 ${Math.round(color.saturation * 50)}`}
                  onPointerDown={(event) => {
                    if (disabled) return;
                    setBand(key);
                    event.currentTarget.setPointerCapture(event.pointerId);
                    drag(key, event);
                  }}
                  onPointerMove={(event) => {
                    if (event.currentTarget.hasPointerCapture(event.pointerId)) drag(key, event);
                  }}
                  onPointerUp={(event) => {
                    if (event.currentTarget.hasPointerCapture(event.pointerId))
                      event.currentTarget.releasePointerCapture(event.pointerId);
                  }}
                  onDoubleClick={() => change(key, { saturation: 0, luminance: 0 })}
                  onFocus={() => setBand(key)}
                  onKeyDown={(event) => {
                    const amount = event.shiftKey ? 0.1 : 0.02;
                    if (event.key === 'Home') {
                      event.preventDefault();
                      change(key, { saturation: 0 });
                      return;
                    }
                    if (!['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(event.key))
                      return;
                    event.preventDefault();
                    change(
                      key,
                      event.key === 'ArrowLeft' || event.key === 'ArrowRight'
                        ? {
                            hue:
                              color.hue +
                              (event.key === 'ArrowRight' ? 1 : -1) * (event.shiftKey ? 10 : 1),
                          }
                        : {
                            saturation: Math.max(
                              0,
                              Math.min(
                                2,
                                color.saturation + (event.key === 'ArrowUp' ? amount : -amount)
                              )
                            ),
                          }
                    );
                  }}
                >
                  <span className="grade-wheel-center" aria-hidden="true" />
                  <span
                    className="grade-wheel-puck"
                    style={{
                      left: `${50 + Math.cos(angle) * color.saturation * 22.5}%`,
                      top: `${50 + Math.sin(angle) * color.saturation * 22.5}%`,
                    }}
                  />
                </div>
              </div>
            );
          })}
      </div>
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
      <AdjustmentSlider
        spec={{ key: 'wheel-luminance', label: '亮度', min: -2, max: 2, scale: 50 }}
        value={value.luminance}
        onChange={(luminance) => change(band, { luminance })}
      />
    </>
  );
}
