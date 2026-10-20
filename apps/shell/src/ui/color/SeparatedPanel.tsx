import type { Adjustments } from '../../editing-client.js';
import {
  defaultSeparated,
  separatedParameters,
  withSeparated,
  type ObservationMode,
} from '../../editing-observation.js';
import { Panel } from '../Panel.js';
import { Icon } from '../Icon.js';
import { AdjustmentSlider, type SliderSpec } from './AdjustmentSlider.js';

const tone: SliderSpec[] = [
  { key: 'exposure', label: '曝光', min: -5, max: 5, step: 0.01, unit: 'EV' },
  ...[
    ['contrast', '对比度'],
    ['highlights', '高光'],
    ['shadows', '阴影'],
    ['whites', '白色'],
    ['blacks', '黑色'],
  ].map(([key, label]) => ({ key: key!, label: label!, min: -2, max: 2, scale: 50 })),
];
const color: SliderSpec[] = [
  { key: 'temperature', label: '冷暖', min: -1, max: 1, scale: 100, tone: 'temperature' },
  { key: 'tint', label: '色调', min: -1, max: 1, scale: 100, tone: 'tint' },
  { key: 'hue', label: '色相', min: -180, max: 180, step: 1 },
  { key: 'saturation', label: '饱和度', min: -1, max: 1, scale: 100 },
];

export function SeparatedPanel({
  mode,
  adjustments,
  onChange,
}: {
  mode: ObservationMode;
  adjustments: Adjustments;
  onChange(value: Adjustments): void;
}) {
  if (mode === 'full') return null;
  const separated = separatedParameters(adjustments);
  const title = mode === 'tone' ? '仅影调' : '仅颜色';
  const values = separated[mode];
  return (
    <Panel
      id={`grade-isolated-${mode}`}
      title={title}
      aside={
        <button
          type="button"
          className="grade-reset"
          title={`重置${title}`}
          aria-label={`重置${title}`}
          onClick={() =>
            onChange(withSeparated(adjustments, { ...separated, [mode]: defaultSeparated()[mode] }))
          }
        >
          <Icon name="refresh" />
        </button>
      }
    >
      <div role="group" aria-label={`${title}调整`}>
        {(mode === 'tone' ? tone : color).map((spec) => (
          <AdjustmentSlider
            key={spec.key}
            spec={spec}
            value={values[spec.key as keyof typeof values] as number}
            onChange={(value) =>
              onChange(
                withSeparated(adjustments, {
                  ...separated,
                  [mode]: { ...values, [spec.key]: value },
                })
              )
            }
          />
        ))}
      </div>
    </Panel>
  );
}
