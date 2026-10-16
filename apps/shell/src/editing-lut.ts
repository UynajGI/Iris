import type { EditDocument } from './editing-client.js';

type Node = EditDocument['nodes'][number];
export type LutOperation = Extract<Node['operation'], { type: 'lut' }>;
type LutNode = Node & { operation: LutOperation };
const role = 'traditional.lut';

export function lutNode(document: EditDocument | null): LutNode | undefined {
  const id = document?.panel_bindings.find((binding) => binding.role === role)?.node_id;
  const node = document?.nodes.find((node) => node.id === id);
  return node?.operation.type === 'lut' ? (node as LutNode) : undefined;
}
/** Only the traditional panel's terminal LUT is kept after newly added local adjustments. */
export function outputLut(document: EditDocument): LutNode | undefined {
  const node = lutNode(document);
  return node?.id === document.output_node && node.inputs.length === 1 ? node : undefined;
}
function validateTarget(document: EditDocument, hash: string) {
  if (!/^[a-f0-9]{64}$/.test(hash)) throw new Error('LUT 资源标识无效');
  const previous = lutNode(document);
  if (!previous && document.panel_bindings.some((binding) => binding.role === role))
    throw new Error('LUT 面板绑定已变更，请检查节点图');
  if (!previous && document.nodes.length >= 256) throw new Error('当前编辑图的节点数量已达上限');
  if (previous && previous.inputs.length !== 1) throw new Error('LUT 节点的连接已变更');
  return previous;
}
export function applyLut(
  document: EditDocument,
  hash: string,
  name: string,
  domain: LutOperation['parameters']['domain']
): EditDocument {
  const previous = validateTarget(document, hash);
  let id = previous?.id ?? 'color-lut';
  for (let i = 1; !previous && document.nodes.some((node) => node.id === id); i++)
    id = `color-lut-${i}`;
  const node: LutNode = {
    id,
    name: name.slice(0, 160),
    enabled: true,
    version: 1,
    inputs: previous?.inputs ?? [document.output_node],
    operation: {
      type: 'lut',
      parameters: {
        resource: hash,
        domain,
        strength: previous?.operation.parameters.strength ?? 1,
      },
    },
  };
  return {
    ...document,
    nodes: previous
      ? document.nodes.map((item) => (item.id === id ? node : item))
      : [...document.nodes, node],
    output_node: previous ? document.output_node : id,
    panel_bindings: previous
      ? document.panel_bindings
      : [...document.panel_bindings, { role, node_id: id }],
  };
}
export function updateLut(
  document: EditDocument,
  patch: Partial<LutOperation['parameters']>,
  enabled?: boolean
): EditDocument {
  const node = lutNode(document);
  if (!node) return document;
  return {
    ...document,
    nodes: document.nodes.map((item) =>
      item.id === node.id
        ? {
            ...item,
            enabled: enabled ?? item.enabled,
            operation: { type: 'lut', parameters: { ...node.operation.parameters, ...patch } },
          }
        : item
    ),
  };
}
export function removeLut(document: EditDocument): EditDocument {
  const node = lutNode(document);
  if (!node) return document;
  if (node.inputs.length !== 1) throw new Error('LUT 节点的连接已变更');
  const input = node.inputs[0]!;
  return {
    ...document,
    nodes: document.nodes
      .filter((item) => item.id !== node.id)
      .map((item) => ({ ...item, inputs: item.inputs.map((id) => (id === node.id ? input : id)) })),
    output_node: document.output_node === node.id ? input : document.output_node,
    panel_bindings: document.panel_bindings.filter((binding) => binding.node_id !== node.id),
    node_positions: Object.fromEntries(
      Object.entries(document.node_positions).filter(([id]) => id !== node.id)
    ),
  };
}
