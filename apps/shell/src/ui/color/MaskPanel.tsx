import { Select } from '../Select.js';
import { useState } from 'react';
import type { EditDocument } from '../../editing-client.js';
import {
  addMask,
  maskLayers,
  maskNames,
  validateMaskBudget,
  removeMask,
  setMask,
  setMaskBlend,
  swatchToWorking,
  workingToSwatch,
  type MaskDefinition,
  type MaskKind,
  createMaskDefinition,
} from '../../editing-masks.js';
import { correctors } from '../../editing-correctors.js';
import { editCorrectorMask } from '../../editing-node-actions.js';
import { Button } from '../components.js';
import { Icon } from '../Icon.js';
import { AdjustmentSlider } from './AdjustmentSlider.js';
import { maskPartAt, replaceMaskPart } from '../../editing-mask-parts.js';
import { MaskParts } from './MaskParts.js';

export type BrushSettings = { radius: number; hardness: number; flow: number; erase: boolean };
export function MaskPanel({
  document,
  disabled,
  selected,
  onSelect,
  onChange,
  brush,
  onBrush,
  selectedPart,
  onPartSelect,
  scope,
}: {
  document: EditDocument | null;
  disabled: boolean;
  selected: string | null;
  onSelect(id: string | null): void;
  onChange(document: EditDocument): void;
  brush: BrushSettings;
  onBrush(value: BrushSettings): void;
  selectedPart: number;
  onPartSelect(index: number): void;
  scope?: string | undefined;
}) {
  const [error, setError] = useState<string | null>(null);
  const group = document && correctors(document).find((g) => g.id === scope);
  const layers = maskLayers(document).filter((layer) => !group || layer.id === group.mask);
  const layer = layers.find((item) => item.id === selected);
  const part = layer && maskPartAt(layer.definition, selectedPart);
  const primary = part?.definition;
  const changeDefinition = (definition: MaskDefinition) => {
    if (!document || !layer) return false;
    try {
      validateMaskBudget(definition);
      onChange(setMask(document, layer, definition));
      setError(null);
      return true;
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      return false;
    }
  };
  const replace = (mask: MaskDefinition) => {
    if (layer && part) changeDefinition(replaceMaskPart(layer.definition, part.path, mask));
  };
  return (
    <section aria-label="蒙版编辑">
      <fieldset disabled={disabled} className="grade-mask-controls">
        {layer && (
          <Button className="quiet" onClick={() => onSelect(null)}>
            <Icon name="back" />
            全部蒙版
          </Button>
        )}
        {!layer && (
          <>
            <div className="grade-mask-list" role="group" aria-label="调整范围">
              {layers.map((item) => (
                <div key={item.id} className="grade-mask-row">
                  <button
                    type="button"
                    aria-pressed={selected === item.id}
                    onClick={() => onSelect(item.id)}
                  >
                    <Icon name="tune" />
                    <span>{item.name}</span>
                  </button>
                  <input
                    type="checkbox"
                    aria-label={`启用${item.name}`}
                    checked={item.visible}
                    onChange={(event) => {
                      if (document)
                        onChange(setMaskBlend(document, item, event.target.checked, item.opacity));
                    }}
                  />
                </div>
              ))}
            </div>
            {(!group || !layers.length) && (
              <Select
                aria-label="新建蒙版"
                value=""
                onValueChange={(nextValue) => {
                  if (!document || !nextValue) return;
                  try {
                    const ready = group && editCorrectorMask(document, group);
                    const result = ready
                      ? {
                          id: ready.mask,
                          document: {
                            ...ready.document,
                            nodes: ready.document.nodes.map((n) =>
                              n.id === ready.mask
                                ? {
                                    ...n,
                                    operation: {
                                      type: 'mask' as const,
                                      parameters: createMaskDefinition(nextValue as MaskKind),
                                    },
                                  }
                                : n
                            ),
                          },
                        }
                      : addMask(document, nextValue as MaskKind);
                    onChange(result.document);
                    onSelect(result.id);
                    setError(null);
                  } catch (reason) {
                    setError(String(reason));
                  }
                }}
              >
                <option value="">新建蒙版</option>
                {Object.entries(maskNames).map(([kind, name]) => (
                  <option key={kind} value={kind}>
                    {name}
                  </option>
                ))}
              </Select>
            )}
          </>
        )}
        {layer && document && (
          <div className="grade-mask-options">
            <label className="grade-mask-name">
              名称
              <input
                aria-label="蒙版名称"
                value={layer.name}
                maxLength={80}
                onChange={(event) =>
                  onChange({
                    ...document,
                    nodes: document.nodes.map((node) =>
                      node.id === layer.id ? { ...node, name: event.target.value } : node
                    ),
                  })
                }
              />
            </label>
            <AdjustmentSlider
              spec={{ key: 'opacity', label: '强度', min: 0, max: 1, scale: 100, reset: 1 }}
              value={layer.opacity}
              onChange={(value) => onChange(setMaskBlend(document, layer, layer.visible, value))}
            />
            <div className="grade-mask-actions">
              <Button
                className="quiet"
                aria-pressed={layer.definition.kind === 'invert'}
                onClick={() =>
                  changeDefinition(
                    layer.definition.kind === 'invert'
                      ? layer.definition.mask
                      : { kind: 'invert', mask: layer.definition }
                  )
                }
              >
                反选
              </Button>
              <Button
                className="quiet"
                onClick={() => {
                  try {
                    onChange(removeMask(document, layer));
                    onSelect(null);
                    setError(null);
                  } catch (reason) {
                    setError(String(reason));
                  }
                }}
              >
                删除蒙版
              </Button>
            </div>
            <MaskParts
              definition={layer.definition}
              selected={selectedPart}
              onSelect={onPartSelect}
              onChange={changeDefinition}
            />
            {primary?.kind === 'brush' && (
              <>
                <div className="segmented" role="group" aria-label="画笔模式">
                  <Button
                    aria-pressed={!brush.erase}
                    onClick={() => onBrush({ ...brush, erase: false })}
                  >
                    画笔
                  </Button>
                  <Button
                    aria-pressed={brush.erase}
                    onClick={() => onBrush({ ...brush, erase: true })}
                  >
                    擦除
                  </Button>
                </div>
                <AdjustmentSlider
                  spec={{
                    key: 'radius',
                    label: '画笔大小',
                    min: 0.005,
                    max: 0.3,
                    scale: 1000,
                    reset: 0.05,
                  }}
                  value={brush.radius}
                  onChange={(value) => onBrush({ ...brush, radius: value })}
                />
                <AdjustmentSlider
                  spec={{ key: 'feather', label: '羽化', min: 0, max: 1, scale: 100, reset: 0.5 }}
                  value={1 - brush.hardness}
                  onChange={(value) => onBrush({ ...brush, hardness: 1 - value })}
                />
                <AdjustmentSlider
                  spec={{ key: 'flow', label: '流量', min: 0, max: 1, scale: 100, reset: 1 }}
                  value={brush.flow}
                  onChange={(value) => onBrush({ ...brush, flow: value })}
                />
              </>
            )}
            {primary?.kind === 'radial_gradient' && (
              <AdjustmentSlider
                spec={{ key: 'feather', label: '羽化', min: 0, max: 1, scale: 100, reset: 0.5 }}
                value={1 - primary.hardness}
                onChange={(value) => replace({ ...primary, hardness: 1 - value })}
              />
            )}
            {primary?.kind === 'luminance_range' && (
              <>
                <AdjustmentSlider
                  spec={{ key: 'low', label: '亮度下限', min: 0, max: 1, scale: 100, reset: 0.2 }}
                  value={primary.low}
                  onChange={(value) => replace({ ...primary, low: Math.min(value, primary.high) })}
                />
                <AdjustmentSlider
                  spec={{ key: 'high', label: '亮度上限', min: 0, max: 1, scale: 100, reset: 0.8 }}
                  value={primary.high}
                  onChange={(value) => replace({ ...primary, high: Math.max(value, primary.low) })}
                />
                <AdjustmentSlider
                  spec={{ key: 'feather', label: '过渡', min: 0, max: 1, scale: 100, reset: 0.1 }}
                  value={primary.feather}
                  onChange={(value) => replace({ ...primary, feather: value })}
                />
              </>
            )}
            {primary?.kind === 'color_range' && (
              <>
                <label className="grade-mask-name">
                  范围颜色
                  <input
                    type="color"
                    aria-label="范围颜色"
                    value={workingToSwatch(primary.color)}
                    onChange={(event) =>
                      replace({ ...primary, color: swatchToWorking(event.target.value) })
                    }
                  />
                </label>
                <AdjustmentSlider
                  spec={{ key: 'radius', label: '颜色范围', min: 0, max: 2, scale: 50, reset: 0.3 }}
                  value={primary.radius}
                  onChange={(value) => replace({ ...primary, radius: value })}
                />
                <AdjustmentSlider
                  spec={{ key: 'feather', label: '过渡', min: 0, max: 1, scale: 100, reset: 0.1 }}
                  value={primary.feather}
                  onChange={(value) => replace({ ...primary, feather: value })}
                />
              </>
            )}
          </div>
        )}
        {error && (
          <p className="inline-error" role="alert">
            {error}
          </p>
        )}
      </fieldset>
    </section>
  );
}
