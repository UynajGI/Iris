import type { components } from './generated/api.js';
import type { EditDocument } from './editing-client.js';
import { defaultAdjustments, defaultDetail } from './editing-document.js';

export type MaskDefinition = components['schemas']['MaskDefinition'];
export type BrushStroke = components['schemas']['BrushStroke'];
export type MaskKind =
  'brush' | 'linear_gradient' | 'radial_gradient' | 'luminance_range' | 'color_range';
export type MaskLayer = {
  id: string;
  name: string;
  definition: MaskDefinition;
  adjustmentId: string;
  detailId: string;
  blendId: string;
  visible: boolean;
  opacity: number;
};
export const maskNames: Record<MaskKind, string> = {
  brush: '画笔',
  linear_gradient: '线性渐变',
  radial_gradient: '径向渐变',
  luminance_range: '亮度范围',
  color_range: '颜色范围',
};
export function primaryMask(mask: MaskDefinition): MaskDefinition {
  if (mask.kind === 'invert' || mask.kind === 'opacity') return primaryMask(mask.mask);
  if (mask.kind === 'combine') return primaryMask(mask.left);
  return mask;
}
export function replacePrimary(mask: MaskDefinition, replacement: MaskDefinition): MaskDefinition {
  if (mask.kind === 'invert' || mask.kind === 'opacity')
    return { ...mask, mask: replacePrimary(mask.mask, replacement) };
  if (mask.kind === 'combine') return { ...mask, left: replacePrimary(mask.left, replacement) };
  return replacement;
}
export function maskLayers(document: EditDocument | null): MaskLayer[] {
  if (!document) return [];
  return document.panel_bindings
    .filter((binding) => binding.role.startsWith('local-adjustments:'))
    .flatMap((binding) => {
      const id = binding.role.slice('local-adjustments:'.length);
      const mask = document.nodes.find((node) => node.id === id);
      const detail = document.panel_bindings.find((item) => item.role === `local-detail:${id}`);
      const blend = document.nodes.find(
        (node) => node.operation.type === 'blend' && node.inputs[2] === id
      );
      if (mask?.operation.type !== 'mask' || !blend || blend.operation.type !== 'blend' || !detail)
        return [];
      return [
        {
          id,
          name: mask.name,
          definition: mask.operation.parameters,
          adjustmentId: binding.node_id,
          detailId: detail.node_id,
          blendId: blend.id,
          visible: blend.enabled,
          opacity: blend.operation.parameters.opacity,
        },
      ];
    });
}
export function addMask(
  document: EditDocument,
  kind: MaskKind,
  identifier: string = crypto.randomUUID()
): { document: EditDocument; id: string } {
  if (document.nodes.length > 252) throw new Error('当前编辑图的蒙版数量已达上限');
  const id = `mask-${identifier}`,
    adjustmentId = `${id}-adjustments`,
    detailId = `${id}-detail`,
    blendId = `${id}-blend`;
  if (document.nodes.some((node) => [id, adjustmentId, detailId, blendId].includes(node.id)))
    throw new Error('蒙版标识重复');
  const definition: MaskDefinition =
    kind === 'brush'
      ? { kind, strokes: [] }
      : kind === 'linear_gradient'
        ? { kind, start: [0.5, 0.25], end: [0.5, 0.75] }
        : kind === 'radial_gradient'
          ? { kind, center: [0.5, 0.5], radii: [0.25, 0.3], hardness: 0.5 }
          : kind === 'luminance_range'
            ? { kind, low: 0.2, high: 0.8, feather: 0.1 }
            : { kind: 'color_range', color: [1, 1, 1], radius: 0.3, feather: 0.1 };
  return {
    id,
    document: {
      ...document,
      nodes: [
        ...document.nodes,
        {
          id,
          name: `${maskNames[kind]} ${maskLayers(document).length + 1}`,
          version: 1,
          enabled: true,
          inputs: [document.output_node],
          operation: { type: 'mask', parameters: definition },
        },
        {
          id: adjustmentId,
          name: '局部影调与颜色',
          version: 1,
          enabled: true,
          inputs: [document.output_node],
          operation: { type: 'adjustments', parameters: defaultAdjustments() },
        },
        {
          id: detailId,
          name: '局部细节',
          version: 1,
          enabled: true,
          inputs: [adjustmentId],
          operation: { type: 'detail', parameters: defaultDetail() },
        },
        {
          id: blendId,
          name: '局部混合',
          version: 1,
          enabled: true,
          inputs: [document.output_node, detailId, id],
          operation: { type: 'blend', parameters: { opacity: 1 } },
        },
      ],
      panel_bindings: [
        ...document.panel_bindings,
        { role: `local-adjustments:${id}`, node_id: adjustmentId },
        { role: `local-detail:${id}`, node_id: detailId },
      ],
      output_node: blendId,
    },
  };
}
export function setMask(
  document: EditDocument,
  layer: MaskLayer,
  definition: MaskDefinition
): EditDocument {
  return {
    ...document,
    nodes: document.nodes.map((node) =>
      node.id === layer.id ? { ...node, operation: { type: 'mask', parameters: definition } } : node
    ),
  };
}
export function setMaskBlend(
  document: EditDocument,
  layer: MaskLayer,
  enabled: boolean,
  opacity: number
): EditDocument {
  return {
    ...document,
    nodes: document.nodes.map((node) =>
      node.id === layer.blendId
        ? { ...node, enabled, operation: { type: 'blend', parameters: { opacity } } }
        : node
    ),
  };
}
export function removeMask(document: EditDocument, layer: MaskLayer): EditDocument {
  const blend = document.nodes.find((node) => node.id === layer.blendId)!;
  const removed = new Set([layer.id, layer.adjustmentId, layer.detailId, layer.blendId]);
  for (const node of document.nodes)
    if (
      !removed.has(node.id) &&
      node.inputs.some((input) => removed.has(input) && input !== layer.blendId)
    )
      throw new Error('该蒙版仍被其他节点使用');
  return {
    ...document,
    output_node: document.output_node === layer.blendId ? blend.inputs[0]! : document.output_node,
    nodes: document.nodes
      .filter((node) => !removed.has(node.id))
      .map((node) => ({
        ...node,
        inputs: node.inputs.map((input) => (input === layer.blendId ? blend.inputs[0]! : input)),
      })),
    panel_bindings: document.panel_bindings.filter((binding) => !removed.has(binding.node_id)),
    node_positions: Object.fromEntries(
      Object.entries(document.node_positions).filter(([id]) => !removed.has(id))
    ),
  };
}

/** User-picked encoded sRGB swatches are converted to the mask's linear Rec.2020 contract. */
export function swatchToWorking(hex: string): number[] {
  const rgb = [1, 3, 5]
    .map((index) => Number.parseInt(hex.slice(index, index + 2), 16) / 255)
    .map((value) => (value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4));
  if (rgb.some((value) => !Number.isFinite(value))) throw new Error('颜色值无效');
  return [
    [0.6274039, 0.329283, 0.0433131],
    [0.0690973, 0.9195404, 0.0113623],
    [0.0163914, 0.0880133, 0.8955953],
  ].map((row) => row.reduce((sum, factor, index) => sum + factor * rgb[index]!, 0));
}
export function workingToSwatch(rgb: number[]): string {
  const values = [
    [1.660491, -0.5876411, -0.0728499],
    [-0.1245505, 1.1328999, -0.0083494],
    [-0.0181508, -0.1005789, 1.1187297],
  ]
    .map((row) => row.reduce((sum, factor, index) => sum + factor * (rgb[index] ?? 0), 0))
    .map((value) => (value <= 0.0031308 ? 12.92 * value : 1.055 * value ** (1 / 2.4) - 0.055))
    .map((value) =>
      Math.round(Math.max(0, Math.min(1, value)) * 255)
        .toString(16)
        .padStart(2, '0')
    );
  return `#${values.join('')}`;
}
export function validateMaskBudget(mask: MaskDefinition) {
  let nodes = 0,
    points = 0;
  const walk = (item: MaskDefinition, depth: number) => {
    nodes++;
    if (depth > 16 || nodes > 256) throw new Error('蒙版组合已达上限，请新建蒙版继续绘制');
    if (item.kind === 'brush')
      points += item.strokes.reduce((sum, stroke) => sum + stroke.points.length, 0);
    else if (item.kind === 'combine') {
      walk(item.left, depth + 1);
      walk(item.right, depth + 1);
    } else if (item.kind === 'invert' || item.kind === 'opacity') walk(item.mask, depth + 1);
    if (points > 4096) throw new Error('画笔控制点已达上限，请新建蒙版继续绘制');
  };
  walk(mask, 0);
}
