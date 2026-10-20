import { useState } from 'react';
import type {
  Adjustments,
  DetailAdjustments,
  EditDocument,
  Geometry,
} from '../../editing-client.js';
import { defaultAdjustments, parameters, updatePanel } from '../../editing-document.js';
import { Panel, PanelGroup } from '../Panel.js';
import {
  adjustmentGroupModified,
  resetAdjustmentGroup,
  type AdjustmentGroup,
} from '../../editing-groups.js';
import { Button } from '../components.js';
import { Icon } from '../Icon.js';
import { AdjustmentSlider, type SliderSpec } from './AdjustmentSlider.js';
import { CurveEditor } from './CurveEditor.js';
import { ColorWheels } from './ColorWheels.js';
import { blackWhiteMix } from '../../editing-document.js';
import type { ObservationMode } from '../../editing-observation.js';
import { SeparatedPanel } from './SeparatedPanel.js';

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
  optics,
  onCrop,
  observation = 'full',
  bypass = null,
  onBypass,
}: {
  document: EditDocument | null;
  disabled: boolean;
  onChange(document: EditDocument): void;
  target?: { adjustmentId: string; detailId: string } | undefined;
  optics?: import('react').ReactNode;
  onCrop?: (() => void) | undefined;
  observation?: ObservationMode;
  bypass?: AdjustmentGroup | null;
  onBypass?: (group: AdjustmentGroup | null) => void;
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
  const section = (group: AdjustmentGroup) => ({
    modified: !target && adjustmentGroupModified(document, group),
    bypassed: !target && bypass === group,
    onBypass: !target && onBypass ? () => onBypass(bypass === group ? null : group) : undefined,
  });
  const resetGroup = (group: AdjustmentGroup) => {
    if (document) onChange(resetAdjustmentGroup(document, group, target));
  };
  return (
    <fieldset className="grade-adjustments" disabled={disabled}>
      <legend className="visually-hidden">传统调色工具</legend>
      <PanelGroup>
        <SeparatedPanel mode={observation} adjustments={a} onChange={changeA} />
        <Panel
          id="grade-basic"
          title="基础调整"
          {...section('basic')}
          aside={<Reset label="基础调整" onClick={() => resetGroup('basic')} />}
        >
          {tone.map((spec) => (
            <AdjustmentSlider
              key={spec.key}
              spec={spec}
              value={a[spec.key as 'exposure']}
              onChange={(value) => changeA({ ...a, [spec.key]: value })}
            />
          ))}
          <h3 className="grade-subheading">颜色</h3>
          <div className="grade-color-mode">
            <span>成片</span>
            <div className="segmented" role="group" aria-label="成片颜色">
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
          <h3 className="grade-subheading">质感</h3>
          {detail.slice(0, 3).map((spec) => (
            <AdjustmentSlider
              key={spec.key}
              spec={spec}
              value={d[spec.key as 'texture']}
              onChange={(value) => changeD({ ...d, [spec.key]: value })}
            />
          ))}
        </Panel>
        <Panel
          id="grade-curve"
          {...section('curve')}
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
          {...section('mixer')}
          title="颜色混合器"
          defaultOpen={false}
          aside={<Reset label="颜色混合器" onClick={() => changeA({ ...a, hsl: [] })} />}
        >
          <HueMixer adjustments={a} onChange={changeA} />
        </Panel>
        <Panel
          id="grade-balance"
          {...section('grading')}
          title="颜色分级"
          defaultOpen={false}
          aside={<Reset label="颜色分级" onClick={() => resetA(['color_balance'])} />}
        >
          <ColorWheels adjustments={a} onChange={changeA} disabled={disabled} />
        </Panel>
        <Panel
          id="grade-detail"
          {...section('detail')}
          title="细节"
          defaultOpen={false}
          aside={<Reset label="细节" onClick={() => resetGroup('detail')} />}
        >
          {detail.slice(3).map((spec) => (
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
          </details>
        </Panel>
        {!target && (
          <>
            <Panel
              id="grade-optics"
              title="光学"
              {...section('optics')}
              defaultOpen={false}
              aside={<Reset label="光学" onClick={() => resetGroup('optics')} />}
            >
              {optics}
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
                    changeG({
                      ...g,
                      radial: g.radial.map((item, i) => (i === index ? value : item)),
                    })
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
            <Panel
              id="grade-transform"
              title="变换"
              {...section('transform')}
              defaultOpen={false}
              aside={<Reset label="变换" onClick={() => resetGroup('transform')} />}
            >
              <AdjustmentSlider
                spec={{ key: 'rotation', label: '拉直', min: -45, max: 45, step: 0.1, unit: '°' }}
                value={g.rotation_degrees}
                onChange={(value) => changeG({ ...g, rotation_degrees: value })}
              />
              <Button className="ghost block" onClick={onCrop}>
                在画布上裁切
              </Button>
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
            </Panel>
          </>
        )}
        <Panel
          id="grade-effects"
          {...section('effects')}
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
            spec={{
              key: 'midpoint',
              label: '暗角范围',
              min: 0,
              max: 0.99,
              scale: 100,
              reset: 0.35,
            }}
            value={a.vignette.midpoint}
            onChange={(value) => changeA({ ...a, vignette: { ...a.vignette, midpoint: value } })}
          />
          <AdjustmentSlider
            spec={{ key: 'feather', label: '暗角羽化', min: 0.1, max: 4, scale: 25, reset: 1 }}
            value={a.vignette.feather}
            onChange={(value) => changeA({ ...a, vignette: { ...a.vignette, feather: value } })}
          />
        </Panel>
      </PanelGroup>
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
      <div className="grade-hues" role="group" aria-label="HSL 颜色">
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
