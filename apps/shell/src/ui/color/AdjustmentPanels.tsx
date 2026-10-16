import { useId, useState } from 'react';
import type {
  Adjustments,
  DetailAdjustments,
  EditDocument,
  Geometry,
} from '../../editing-client.js';
import {
  defaultAdjustments,
  defaultDetail,
  defaultGeometry,
  parameters,
  updatePanel,
} from '../../editing-document.js';
import { Panel } from '../Panel.js';
import { Button } from '../components.js';
import { Icon } from '../Icon.js';
import { CurveEditor } from './CurveEditor.js';
import { blackWhiteMix } from '../../editing-document.js';

type SliderSpec = {
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
  { key: 'vibrance', label: '自然饱和度', min: -1, max: 1, scale: 100 },
  { key: 'saturation', label: '饱和度', min: -1, max: 1, scale: 100 },
];
const detail: SliderSpec[] = [
  { key: 'texture', label: '纹理', min: -1, max: 1, scale: 100 },
  { key: 'clarity', label: '清晰度', min: -1, max: 1, scale: 100 },
  { key: 'dehaze', label: '去雾', min: -1, max: 1, scale: 100 },
  { key: 'sharpening', label: '锐化', min: 0, max: 3, scale: 100 },
  { key: 'luminance_denoise', label: '亮度降噪', min: 0, max: 1, scale: 100 },
  { key: 'chroma_denoise', label: '颜色降噪', min: 0, max: 1, scale: 100 },
];

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
    </div>
  );
}

function Reset({ label, onClick }: { label: string; onClick(): void }) {
  return (
    <button
      type="button"
      className="grade-reset"
      aria-label={`重置${label}`}
      title={`重置${label}`}
      onClick={onClick}
    >
      <Icon name="refresh" />
    </button>
  );
}

export function AdjustmentPanels({
  document,
  disabled,
  onChange,
  target,
}: {
  document: EditDocument | null;
  disabled: boolean;
  onChange(document: EditDocument): void;
  target?: { adjustmentId: string; detailId: string } | undefined;
}) {
  const localA =
    target && document?.nodes.find((node) => node.id === target.adjustmentId)?.operation;
  const localD = target && document?.nodes.find((node) => node.id === target.detailId)?.operation;
  const a =
    localA && localA.type === 'adjustments'
      ? localA.parameters
      : parameters(document, 'adjustments');
  const d = localD && localD.type === 'detail' ? localD.parameters : parameters(document, 'detail');
  const g = parameters(document, 'geometry');
  const changeA = (value: Adjustments) => {
    if (document)
      onChange(
        target
          ? {
              ...document,
              nodes: document.nodes.map((node) =>
                node.id === target.adjustmentId
                  ? { ...node, operation: { type: 'adjustments', parameters: value } }
                  : node
              ),
            }
          : updatePanel(document, { type: 'adjustments', parameters: value })
      );
  };
  const changeD = (value: DetailAdjustments) => {
    if (document)
      onChange(
        target
          ? {
              ...document,
              nodes: document.nodes.map((node) =>
                node.id === target.detailId
                  ? { ...node, operation: { type: 'detail', parameters: value } }
                  : node
              ),
            }
          : updatePanel(document, { type: 'detail', parameters: value })
      );
  };
  const changeG = (value: Geometry) => {
    if (document) onChange(updatePanel(document, { type: 'geometry', parameters: value }));
  };
  const resetA = (keys: (keyof Adjustments)[]) => {
    const defaults = defaultAdjustments();
    changeA({ ...a, ...Object.fromEntries(keys.map((key) => [key, defaults[key]])) });
  };
  return (
    <fieldset className="grade-adjustments" disabled={disabled}>
      <legend className="visually-hidden">传统调色工具</legend>
      <Panel
        id="grade-light"
        title="影调"
        aside={
          <Reset
            label="影调"
            onClick={() => resetA(tone.map((item) => item.key as keyof Adjustments))}
          />
        }
      >
        {tone.map((spec) => (
          <AdjustmentSlider
            key={spec.key}
            spec={spec}
            value={a[spec.key as 'exposure']}
            onChange={(value) => changeA({ ...a, [spec.key]: value })}
          />
        ))}
      </Panel>
      <Panel
        id="grade-color"
        title="颜色"
        aside={
          <Reset
            label="颜色"
            onClick={() => resetA(color.map((item) => item.key as keyof Adjustments))}
          />
        }
      >
        <div className="grade-color-mode">
          <span>模式</span>
          <div className="segmented" role="group" aria-label="颜色模式">
            <Button
              aria-pressed={!a.black_and_white.enabled}
              onClick={() =>
                changeA({ ...a, black_and_white: { ...a.black_and_white, enabled: false } })
              }
            >
              彩色
            </Button>
            <Button
              aria-pressed={a.black_and_white.enabled}
              onClick={() =>
                changeA({ ...a, black_and_white: { ...a.black_and_white, enabled: true } })
              }
            >
              黑白
            </Button>
          </div>
        </div>
        {color.map((spec) => (
          <AdjustmentSlider
            key={spec.key}
            spec={spec}
            value={a[spec.key as 'temperature']}
            onChange={(value) => changeA({ ...a, [spec.key]: value })}
          />
        ))}
        {a.black_and_white.enabled && <BlackWhiteMixer adjustments={a} onChange={changeA} />}
      </Panel>
      <Panel
        id="grade-balance"
        title="色彩平衡"
        defaultOpen={false}
        aside={<Reset label="色彩平衡" onClick={() => resetA(['color_balance'])} />}
      >
        <ColorBalanceControls adjustments={a} onChange={changeA} />
      </Panel>
      <Panel
        id="grade-curve"
        title="曲线"
        defaultOpen={false}
        aside={
          <Reset
            label="曲线"
            onClick={() =>
              resetA(['master_curve', 'red_curve', 'green_curve', 'blue_curve', 'tone_curve'])
            }
          />
        }
      >
        <CurveEditor adjustments={a} onChange={changeA} />
        {(['shadows', 'darks', 'lights', 'highlights'] as const).map((key, index) => (
          <AdjustmentSlider
            key={key}
            spec={{
              key,
              label: ['暗部', '深色', '浅色', '亮部'][index]!,
              min: -2,
              max: 2,
              scale: 50,
            }}
            value={a.tone_curve[key]}
            onChange={(value) => changeA({ ...a, tone_curve: { ...a.tone_curve, [key]: value } })}
          />
        ))}
      </Panel>
      <Panel
        id="grade-hsl"
        title="混色"
        defaultOpen={false}
        aside={<Reset label="混色" onClick={() => changeA({ ...a, hsl: [] })} />}
      >
        <HueMixer adjustments={a} onChange={changeA} />
      </Panel>
      <Panel
        id="grade-detail"
        title="细节"
        defaultOpen={false}
        aside={<Reset label="细节" onClick={() => changeD(defaultDetail())} />}
      >
        {detail.map((spec) => (
          <AdjustmentSlider
            key={spec.key}
            spec={spec}
            value={d[spec.key as 'texture']}
            onChange={(value) => changeD({ ...d, [spec.key]: value })}
          />
        ))}
        <details className="grade-advanced">
          <summary>高级细节</summary>
          <AdjustmentSlider
            spec={{ key: 'threshold', label: '锐化阈值', min: 0, max: 0.25, scale: 400 }}
            value={d.sharpening_threshold}
            onChange={(value) => changeD({ ...d, sharpening_threshold: value })}
          />
          <AdjustmentSlider
            spec={{
              key: 'sigma',
              label: '降噪范围',
              min: 0.001,
              max: 1,
              step: 0.001,
              scale: 100,
              reset: 0.04,
            }}
            value={d.denoise_sigma}
            onChange={(value) => changeD({ ...d, denoise_sigma: value })}
          />
          <AdjustmentSlider
            spec={{ key: 'atmosphere', label: '大气光', min: 0.01, max: 16, step: 0.01, reset: 1 }}
            value={d.atmospheric_light}
            onChange={(value) => changeD({ ...d, atmospheric_light: value })}
          />
        </details>
      </Panel>
      <Panel
        id="grade-effects"
        title="效果"
        defaultOpen={false}
        aside={<Reset label="效果" onClick={() => resetA(['grain', 'vignette'])} />}
      >
        <AdjustmentSlider
          spec={{ key: 'vignette', label: '暗角', min: -4, max: 4, scale: 25 }}
          value={a.vignette.amount}
          onChange={(value) => changeA({ ...a, vignette: { ...a.vignette, amount: value } })}
        />
        <AdjustmentSlider
          spec={{ key: 'grain', label: '颗粒', min: 0, max: 0.25, scale: 400 }}
          value={a.grain.amount}
          onChange={(value) => changeA({ ...a, grain: { ...a.grain, amount: value } })}
        />
        <AdjustmentSlider
          spec={{ key: 'midpoint', label: '暗角范围', min: 0, max: 0.99, scale: 100, reset: 0.35 }}
          value={a.vignette.midpoint}
          onChange={(value) => changeA({ ...a, vignette: { ...a.vignette, midpoint: value } })}
        />
        <AdjustmentSlider
          spec={{ key: 'feather', label: '暗角羽化', min: 0.1, max: 4, scale: 25, reset: 1 }}
          value={a.vignette.feather}
          onChange={(value) => changeA({ ...a, vignette: { ...a.vignette, feather: value } })}
        />
      </Panel>
      {!target && (
        <Panel
          id="grade-geometry"
          title="几何与光学"
          defaultOpen={false}
          aside={<Reset label="几何与光学" onClick={() => changeG(defaultGeometry())} />}
        >
          <AdjustmentSlider
            spec={{ key: 'rotation', label: '拉直', min: -45, max: 45, step: 0.1, unit: '°' }}
            value={g.rotation_degrees}
            onChange={(value) => changeG({ ...g, rotation_degrees: value })}
          />
          <CropControls geometry={g} onChange={changeG} />
          {[6, 7].map((index) => (
            <AdjustmentSlider
              key={index}
              spec={{
                key: `perspective-${index}`,
                label: index === 6 ? '水平透视' : '垂直透视',
                min: -0.4,
                max: 0.4,
                step: 0.001,
                scale: 250,
              }}
              value={g.perspective[index]!}
              onChange={(value) =>
                changeG({
                  ...g,
                  perspective: g.perspective.map((item, i) => (i === index ? value : item)),
                })
              }
            />
          ))}
          {[0, 1].map((index) => (
            <AdjustmentSlider
              key={index}
              spec={{
                key: `radial-${index}`,
                label: index ? '畸变二阶' : '畸变',
                min: -0.2,
                max: 0.2,
                step: 0.001,
                scale: 100,
              }}
              value={g.radial[index]!}
              onChange={(value) =>
                changeG({ ...g, radial: g.radial.map((item, i) => (i === index ? value : item)) })
              }
            />
          ))}
          <AdjustmentSlider
            spec={{
              key: 'optical-vignette',
              label: '暗角校正',
              min: -4,
              max: 4,
              step: 0.01,
              unit: 'EV',
            }}
            value={g.vignette_stops}
            onChange={(value) => changeG({ ...g, vignette_stops: value })}
          />
          {[0, 1].map((index) => (
            <AdjustmentSlider
              key={index}
              spec={{
                key: `ca-${index}`,
                label: index === 0 ? '红边校正' : '蓝边校正',
                min: -0.1,
                max: 0.1,
                step: 0.001,
                scale: 1000,
              }}
              value={g.chromatic_aberration[index]!}
              onChange={(value) =>
                changeG({
                  ...g,
                  chromatic_aberration: g.chromatic_aberration.map((item, i) =>
                    i === index ? value : item
                  ),
                })
              }
            />
          ))}
        </Panel>
      )}
    </fieldset>
  );
}

function BlackWhiteMixer({
  adjustments: a,
  onChange,
}: {
  adjustments: Adjustments;
  onChange(value: Adjustments): void;
}) {
  return (
    <div className="grade-bw-mixer" title="三个通道的总比例保持 100%">
      {['红色占比', '绿色占比', '蓝色占比'].map((label, index) => (
        <AdjustmentSlider
          key={label}
          spec={{
            key: `bw-${index}`,
            label,
            min: -2,
            max: 2,
            scale: 100,
            reset: defaultAdjustments().black_and_white.mix[index]!,
          }}
          value={a.black_and_white.mix[index]!}
          onChange={(value) =>
            onChange({
              ...a,
              black_and_white: {
                ...a.black_and_white,
                mix: blackWhiteMix(a.black_and_white.mix, index, value),
              },
            })
          }
        />
      ))}
    </div>
  );
}
function ColorBalanceControls({
  adjustments: a,
  onChange,
}: {
  adjustments: Adjustments;
  onChange(value: Adjustments): void;
}) {
  const [band, setBand] = useState<'shadows' | 'midtones' | 'highlights'>('midtones');
  return (
    <>
      <div className="segmented grade-balance-tabs" role="group" aria-label="色彩平衡范围">
        {(
          [
            ['shadows', '阴影'],
            ['midtones', '中间调'],
            ['highlights', '高光'],
          ] as const
        ).map(([key, name]) => (
          <Button key={key} aria-pressed={band === key} onClick={() => setBand(key)}>
            {name}
          </Button>
        ))}
      </div>
      {['红色', '绿色', '蓝色'].map((label, index) => (
        <AdjustmentSlider
          key={label}
          spec={{ key: `balance-${index}`, label, min: -2, max: 2, scale: 50 }}
          value={a.color_balance[band][index]!}
          onChange={(value) =>
            onChange({
              ...a,
              color_balance: {
                ...a.color_balance,
                [band]: a.color_balance[band].map((item, i) => (i === index ? value : item)),
              },
            })
          }
        />
      ))}
    </>
  );
}
function CropControls({
  geometry: g,
  onChange,
}: {
  geometry: Geometry;
  onChange(value: Geometry): void;
}) {
  const edges = [g.crop[0]!, 1 - g.crop[0]! - g.crop[2]!, g.crop[1]!, 1 - g.crop[1]! - g.crop[3]!];
  return (
    <details className="grade-advanced">
      <summary>裁切边界</summary>
      {['左侧裁切', '右侧裁切', '顶部裁切', '底部裁切'].map((label, index) => {
        const opposite = index % 2 === 0 ? index + 1 : index - 1;
        return (
          <AdjustmentSlider
            key={label}
            spec={{
              key: `crop-${index}`,
              label,
              min: 0,
              max: Math.max(0, 0.99 - edges[opposite]!),
              step: 0.001,
              scale: 100,
            }}
            value={Math.max(0, edges[index]!)}
            onChange={(value) => {
              const next = [...edges];
              next[index] = value;
              onChange({
                ...g,
                crop: [next[0]!, next[2]!, 1 - next[0]! - next[1]!, 1 - next[2]! - next[3]!],
              });
            }}
          />
        );
      })}
    </details>
  );
}

const hues = [
  ['红', 0],
  ['橙', 30],
  ['黄', 60],
  ['绿', 120],
  ['青', 180],
  ['蓝', 240],
  ['紫', 280],
  ['洋红', 320],
] as const;
function HueMixer({
  adjustments: a,
  onChange,
}: {
  adjustments: Adjustments;
  onChange(value: Adjustments): void;
}) {
  const [center, setCenter] = useState(0);
  const band = a.hsl.find((item) => item.center === center) ?? {
    center,
    width: 45,
    hue_shift: 0,
    saturation: 0,
    luminance: 0,
  };
  const patch = (key: 'hue_shift' | 'saturation' | 'luminance', value: number) =>
    onChange({
      ...a,
      hsl: [...a.hsl.filter((item) => item.center !== center), { ...band, [key]: value }].sort(
        (left, right) => left.center - right.center
      ),
    });
  return (
    <>
      <div className="grade-hues" role="group" aria-label="混色颜色">
        {hues.map(([name, hue]) => (
          <button
            type="button"
            key={hue}
            aria-label={name}
            aria-pressed={center === hue}
            title={name}
            style={{ '--hue': hue } as React.CSSProperties}
            onClick={() => setCenter(hue)}
          />
        ))}
      </div>
      <AdjustmentSlider
        spec={{ key: 'hue', label: '色相', min: -180, max: 180, step: 1 }}
        value={band.hue_shift}
        onChange={(value) => patch('hue_shift', value)}
      />
      <AdjustmentSlider
        spec={{ key: 'saturation', label: '饱和度', min: -1, max: 1, scale: 100 }}
        value={band.saturation}
        onChange={(value) => patch('saturation', value)}
      />
      <AdjustmentSlider
        spec={{ key: 'luminance', label: '明亮度', min: -1, max: 1, scale: 100 }}
        value={band.luminance}
        onChange={(value) => patch('luminance', value)}
      />
    </>
  );
}
