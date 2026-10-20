import type { Adjustments, DetailAdjustments, EditDocument, Geometry } from './editing-client.js';
import {
  defaultAdjustments,
  defaultDetail,
  defaultGeometry,
  panelNode,
  parameters,
  updatePanel,
} from './editing-document.js';
import { lensProfileNode } from './editing-lens.js';

export type AdjustmentGroup =
  'basic' | 'curve' | 'mixer' | 'grading' | 'detail' | 'optics' | 'transform' | 'effects';
const adjustmentKeys: Partial<Record<AdjustmentGroup, (keyof Adjustments)[]>> = {
  basic: [
    'exposure',
    'contrast',
    'highlights',
    'shadows',
    'whites',
    'blacks',
    'temperature',
    'tint',
    'saturation',
    'vibrance',
    'black_and_white',
  ],
  curve: ['master_curve', 'red_curve', 'green_curve', 'blue_curve', 'tone_curve'],
  mixer: ['hsl'],
  grading: ['color_balance'],
  effects: ['grain', 'vignette'],
};
const detailKeys: Partial<Record<AdjustmentGroup, (keyof DetailAdjustments)[]>> = {
  basic: ['texture', 'clarity', 'dehaze', 'atmospheric_light'],
  detail: [
    'sharpening',
    'sharpening_threshold',
    'luminance_denoise',
    'chroma_denoise',
    'denoise_sigma',
  ],
};
const geometryKeys: Partial<Record<AdjustmentGroup, (keyof Geometry)[]>> = {
  optics: ['radial', 'chromatic_aberration', 'vignette_stops'],
  transform: ['rotation_degrees', 'perspective'],
};
function replace<T extends object>(value: T, defaults: T, keys: (keyof T)[]): T {
  return { ...value, ...Object.fromEntries(keys.map((key) => [key, defaults[key]])) };
}

/** A disposable parameter projection. Never change the source binding, crop or masks. */
export function resetAdjustmentGroup(
  document: EditDocument,
  group: AdjustmentGroup,
  target?: { adjustmentId: string; detailId: string }
): EditDocument {
  if (target) {
    const projected = {
      ...document,
      panel_bindings: [
        { role: 'basic', node_id: target.adjustmentId },
        { role: 'detail', node_id: target.detailId },
        ...document.panel_bindings.filter(
          (item) =>
            !['basic', 'detail', 'traditional.adjustments', 'traditional.detail'].includes(
              item.role
            )
        ),
      ],
    };
    return { ...resetAdjustmentGroup(projected, group), panel_bindings: document.panel_bindings };
  }
  let result = document;
  const a = adjustmentKeys[group];
  const d = detailKeys[group];
  const g = geometryKeys[group];
  if (a && panelNode(result, 'adjustments'))
    result = updatePanel(result, {
      type: 'adjustments',
      parameters: replace(parameters(result, 'adjustments'), defaultAdjustments(), a),
    });
  if (d && panelNode(result, 'detail'))
    result = updatePanel(result, {
      type: 'detail',
      parameters: replace(parameters(result, 'detail'), defaultDetail(), d),
    });
  if (g && panelNode(result, 'geometry'))
    result = updatePanel(result, {
      type: 'geometry',
      parameters: replace(parameters(result, 'geometry'), defaultGeometry(), g),
    });
  const lens = group === 'optics' && lensProfileNode(result);
  if (lens)
    result = {
      ...result,
      nodes: result.nodes.map((node) => (node.id === lens.id ? { ...node, enabled: false } : node)),
    };
  return result;
}

export function bypassAdjustmentGroup(
  document: EditDocument,
  group: AdjustmentGroup | null
): EditDocument {
  return group ? resetAdjustmentGroup(document, group) : document;
}

export function adjustmentGroupModified(
  document: EditDocument | null,
  group: AdjustmentGroup
): boolean {
  if (!document) return false;
  return JSON.stringify(document) !== JSON.stringify(resetAdjustmentGroup(document, group));
}
