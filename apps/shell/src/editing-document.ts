import type { Adjustments, DetailAdjustments, EditDocument, Geometry } from './editing-client.js';
import type { components } from './generated/api.js';

type Operation = components['schemas']['Operation'];
export type EditableKind = 'adjustments' | 'detail' | 'geometry';
export function blackWhiteMix(values: number[], channel: number, value: number): number[] {
  const next = [...values];
  const others = [0, 1, 2].filter((index) => index !== channel);
  const chosen = Math.max(-2, Math.min(2, value));
  const target = 1 - chosen;
  const difference = target - values[others[0]!]! - values[others[1]!]!;
  let first = Math.max(-2, Math.min(2, values[others[0]!]! + difference / 2));
  let second = target - first;
  if (second < -2) {
    second = -2;
    first = target + 2;
  }
  if (second > 2) {
    second = 2;
    first = target - 2;
  }
  next[channel] = chosen;
  next[others[0]!] = first;
  next[others[1]!] = second;
  return next;
}
export function sourceDocument(document: EditDocument): EditDocument {
  const source = document.nodes.find((node) => node.operation.type === 'source');
  if (!source) throw new Error('编辑文档缺少原片节点');
  return {
    ...document,
    nodes: [source],
    output_node: source.id,
    panel_bindings: [],
    node_positions: {},
  };
}
export const defaultAdjustments = (): Adjustments => ({
  exposure: 0,
  contrast: 0,
  contrast_pivot: 0.18,
  highlights: 0,
  shadows: 0,
  whites: 0,
  blacks: 0,
  temperature: 0,
  tint: 0,
  saturation: 0,
  vibrance: 0,
  tone_curve: { shadows: 0, darks: 0, lights: 0, highlights: 0 },
  master_curve: { points: [] },
  red_curve: { points: [] },
  green_curve: { points: [] },
  blue_curve: { points: [] },
  hsl: [],
  color_balance: { shadows: [0, 0, 0], midtones: [0, 0, 0], highlights: [0, 0, 0] },
  black_and_white: { enabled: false, mix: [0.2627, 0.678, 0.0593] },
  grain: { amount: 0, seed: 0 },
  vignette: { amount: 0, feather: 1, midpoint: 0.35 },
});
export const defaultDetail = (): DetailAdjustments => ({
  texture: 0,
  clarity: 0,
  dehaze: 0,
  atmospheric_light: 1,
  sharpening: 0,
  sharpening_threshold: 0,
  luminance_denoise: 0,
  chroma_denoise: 0,
  denoise_sigma: 0.04,
});
export const defaultGeometry = (): Geometry => ({
  crop: [0, 0, 1, 1],
  rotation_degrees: 0,
  perspective: [1, 0, 0, 0, 1, 0, 0, 0, 1],
  radial: [0, 0],
  chromatic_aberration: [0, 0],
  vignette_stops: 0,
});
export function resetDocument(document: EditDocument): EditDocument {
  const source = document.nodes.find((node) => node.operation.type === 'source');
  if (!source) throw new Error('编辑文档缺少原片节点');
  return {
    ...document,
    source: { ...document.source, raw_white_balance: null },
    nodes: [
      source,
      {
        id: 'basic',
        name: '影调与颜色',
        version: 1,
        enabled: true,
        inputs: [source.id],
        operation: { type: 'adjustments', parameters: defaultAdjustments() },
      },
      {
        id: 'detail',
        name: '细节',
        version: 1,
        enabled: true,
        inputs: ['basic'],
        operation: { type: 'detail', parameters: defaultDetail() },
      },
      {
        id: 'geometry',
        name: '几何与光学',
        version: 1,
        enabled: true,
        inputs: ['detail'],
        operation: { type: 'geometry', parameters: defaultGeometry() },
      },
    ],
    output_node: 'geometry',
    output: { ...document.output, tone_map: { kind: 'none' } },
    panel_bindings: [
      { role: 'basic', node_id: 'basic' },
      { role: 'detail', node_id: 'detail' },
      { role: 'geometry', node_id: 'geometry' },
    ],
    node_positions: {},
  };
}

/** A panel edits only its own named node; arbitrary node graphs remain intact. */
export function panelNode(document: EditDocument, kind: EditableKind) {
  const role = kind === 'adjustments' ? 'basic' : kind;
  const binding = document.panel_bindings.find(
    (item) => item.role === role || item.role === `traditional.${kind}`
  );
  return (
    document.nodes.find((node) => node.id === binding?.node_id && node.operation.type === kind) ??
    document.nodes.find((node) => node.id === `traditional-${kind}` && node.operation.type === kind)
  );
}
export function parameters(document: EditDocument | null, kind: 'adjustments'): Adjustments;
export function parameters(document: EditDocument | null, kind: 'detail'): DetailAdjustments;
export function parameters(document: EditDocument | null, kind: 'geometry'): Geometry;
export function parameters(document: EditDocument | null, kind: EditableKind) {
  const node = document && panelNode(document, kind);
  if (node && 'parameters' in node.operation) return node.operation.parameters;
  return kind === 'adjustments'
    ? defaultAdjustments()
    : kind === 'detail'
      ? defaultDetail()
      : defaultGeometry();
}
export function updatePanel(
  document: EditDocument,
  operation: Extract<Operation, { type: EditableKind }>
): EditDocument {
  const existing = panelNode(document, operation.type);
  if (existing)
    return {
      ...document,
      nodes: document.nodes.map((node) =>
        node.id === existing.id ? { ...node, operation } : node
      ),
    };
  const id = `traditional-${operation.type}`;
  if (document.nodes.some((node) => node.id === id))
    throw new Error('调色节点名称冲突，请使用新版本');
  return {
    ...document,
    nodes: [
      ...document.nodes,
      {
        id,
        name: operation.type,
        version: 1,
        enabled: true,
        inputs: [document.output_node],
        operation,
      },
    ],
    output_node: id,
    panel_bindings: [
      ...document.panel_bindings,
      { role: `traditional.${operation.type}`, node_id: id },
    ],
  };
}
