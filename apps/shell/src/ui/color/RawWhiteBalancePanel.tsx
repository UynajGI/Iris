import { Select } from '../Select.js';
import { Panel } from '../Panel.js';
import { Button } from '../components.js';
import { AdjustmentSlider } from './AdjustmentPanels.js';
import type { RawWhiteBalanceModel } from './useRawWhiteBalance.js';

export function RawWhiteBalancePanel({
  model,
  disabled,
  asShot,
  picking,
  onPick,
}: {
  model: RawWhiteBalanceModel;
  disabled: boolean;
  asShot: boolean;
  picking: boolean;
  onPick(): void;
}) {
  const { reading } = model;
  const change = (kelvin: number, tint_delta_v: number) =>
    model.request({
      kind: 'temperature',
      kelvin,
      tint_delta_v,
      locus: 'planckian',
    });
  return (
    <Panel id="grade-raw-white-balance" title="白平衡">
      <fieldset className="grade-adjustments grade-white-balance" disabled={disabled}>
        <legend className="visually-hidden">RAW 白平衡</legend>
        <div className="grade-white-balance-actions">
          <label className="visually-hidden" htmlFor="raw-white-balance-preset">
            白平衡预设
          </label>
          <Select
            id="raw-white-balance-preset"
            value={asShot && !model.pending ? 'as_shot' : 'custom'}
            onValueChange={(nextValue) => {
              if (nextValue === 'as_shot') model.request({ kind: 'as_shot' });
              else change(Number(nextValue), 0);
            }}
          >
            <option value="as_shot">拍摄时</option>
            <option value="custom" disabled>
              自定
            </option>
            <option value="5500">日光 · 5500 K</option>
            <option value="6500">阴天 · 6500 K</option>
            <option value="7500">阴影 · 7500 K</option>
            <option value="2850">白炽灯 · 2850 K</option>
          </Select>
          <Button
            className="quiet"
            aria-label="白平衡吸管"
            aria-pressed={picking}
            disabled={model.loading}
            onClick={onPick}
          >
            {picking ? '取消取样' : '吸管'}
          </Button>
        </div>
        {reading ? (
          <>
            <AdjustmentSlider
              spec={{
                key: 'kelvin',
                label: '色温',
                min: 1667,
                max: 25000,
                step: 1,
                unit: 'K',
                reset: 5500,
                tone: 'temperature',
              }}
              value={Math.round(reading.kelvin)}
              onChange={(value) => change(value, reading.tint_delta_v)}
            />
            <AdjustmentSlider
              spec={{
                key: 'raw-tint',
                label: '色偏',
                min: -0.02,
                max: 0.02,
                step: 0.0001,
                scale: 5000,
                tone: 'tint',
              }}
              value={reading.tint_delta_v}
              onChange={(value) => change(reading.kelvin, value)}
            />
          </>
        ) : (
          <p className="muted" role="status">
            {model.loading ? '正在读取白平衡' : '当前白平衡无法换算为色温，可选择预设或取样'}
          </p>
        )}
        {model.error && (
          <div className="grade-error" role="alert">
            <span>{model.error}</span>
            <Button className="quiet" onClick={model.retry}>
              重试
            </Button>
          </div>
        )}
      </fieldset>
    </Panel>
  );
}
