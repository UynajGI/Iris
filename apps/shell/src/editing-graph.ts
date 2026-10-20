import type { EditDocument } from './editing-client.js';
import { defaultAdjustments, defaultDetail, defaultGeometry } from './editing-document.js';
import { canonicalJson } from './editing-equality.js';
export type EditNode = EditDocument['nodes'][number];
export type SerialKind = 'adjustments' | 'detail' | 'geometry';
export function selectedGraphNode(document: EditDocument | null, id: string | null) {
  return (
    document?.nodes.find((node) => node.id === (id ?? document.output_node)) ??
    document?.nodes.find((node) => node.id === document.output_node)
  );
}
export const nodeNames: Record<EditNode['operation']['type'], string> = {
  source: '原片',
  adjustments: '调色',
  detail: '细节',
  geometry: '几何与光学',
  mask: '蒙版',
  blend: '混合',
  lut: 'LUT',
  alpha_mask: 'Alpha 输入',
  key_mix: 'Key 混合',
  set_alpha: 'Alpha 输出',
  parallel_mix: '并行混合',
  layer_mix: '图层混合',
};
const ports: Partial<Record<EditNode['operation']['type'], string[]>> = {
  source: [],
  blend: ['底图', '调整', '蒙版'],
  key_mix: ['输入', '局部 Key', '外部 Key'],
  set_alpha: ['RGB', 'Key'],
  parallel_mix: ['共同输入', 'RGB 1', 'RGB 2', 'Key 1', 'Key 2'],
  layer_mix: ['底图', '上层', 'Key'],
};
export const portNames = (node: EditNode): string[] => ports[node.operation.type] ?? ['输入'];
export const isKeyOutput = (node: EditNode) =>
  ['mask', 'alpha_mask', 'key_mix'].includes(node.operation.type);
export const isKeyInput = (node: EditNode, index: number) => {
  const kind = node.operation.type;
  if (kind === 'blend' || kind === 'layer_mix') return index === 2;
  if (kind === 'set_alpha' || kind === 'key_mix') return index > 0;
  return kind === 'parallel_mix' && index >= 3;
};
export function validateGraph(document: EditDocument) {
  if (!document.nodes.length || document.nodes.length > 256) throw Error('节点数量超出限制');
  const nodes = new Map(document.nodes.map((node) => [node.id, node]));
  if (nodes.size !== document.nodes.length) throw Error('节点标识重复');
  if (document.nodes.filter((n) => n.operation.type === 'source').length !== 1)
    throw Error('必须保留一个原片节点');
  const done = new Map<string, string>(),
    visiting = new Set<string>();
  const visit = (id: string): string => {
    if (done.has(id)) return done.get(id)!;
    const node = nodes.get(id);
    if (!node) throw Error('连接的节点不存在');
    if (visiting.has(id)) throw Error('不能连接成循环');
    visiting.add(id);
    if (node.inputs.length !== portNames(node).length) throw Error('节点输入不完整');
    const grids = node.inputs.map((parent, index) => {
      const source = nodes.get(parent);
      const mask = isKeyInput(node, index);
      if (!source || isKeyOutput(source) !== mask) throw Error('图像与蒙版端口不能混接');
      return visit(parent);
    });
    let grid = grids[0] ?? '';
    if (
      node.enabled &&
      ['blend', 'parallel_mix', 'layer_mix', 'key_mix', 'set_alpha'].includes(
        node.operation.type
      ) &&
      grids.some((value) => value !== grid)
    )
      throw Error('混合图像与蒙版需要相同的裁切和几何坐标');
    if (node.enabled && node.operation.type === 'geometry') {
      const spatial = { ...node.operation.parameters, vignette_stops: 0 };
      if (canonicalJson(spatial) !== canonicalJson(defaultGeometry()))
        grid += canonicalJson(spatial);
    }
    visiting.delete(id);
    done.set(id, grid);
    return grid;
  };
  document.nodes.forEach((node) => visit(node.id));
  if (!nodes.has(document.output_node) || isKeyOutput(nodes.get(document.output_node)!))
    throw Error('输出必须连接图像节点');
  return document;
}
export function connectNode(document: EditDocument, source: string, target: string, port: number) {
  const node = document.nodes.find((n) => n.id === target);
  if (!node || port < 0 || port >= node.inputs.length) throw Error('无效输入端口');
  return validateGraph({
    ...document,
    nodes: document.nodes.map((n) =>
      n.id === target ? { ...n, inputs: n.inputs.map((id, i) => (i === port ? source : id)) } : n
    ),
  });
}
export function addSerialNode(
  document: EditDocument,
  parent: string,
  kind: SerialKind,
  id: string = crypto.randomUUID()
) {
  const source = document.nodes.find((n) => n.id === parent);
  if (!source || source.operation.type === 'mask') throw Error('请选择一个图像节点');
  const operation: EditNode['operation'] =
    kind === 'adjustments'
      ? { type: kind, parameters: defaultAdjustments() }
      : kind === 'detail'
        ? { type: kind, parameters: defaultDetail() }
        : { type: kind, parameters: defaultGeometry() };
  const node: EditNode = {
    id,
    name: nodeNames[kind],
    version: 1,
    enabled: true,
    inputs: [parent],
    operation,
  };
  const nodes = document.nodes.map((n) => ({
    ...n,
    inputs: n.inputs.map((input) => (input === parent ? id : input)),
  }));
  const point = document.node_positions[parent];
  return validateGraph({
    ...document,
    nodes: [...nodes, node],
    output_node: document.output_node === parent ? id : document.output_node,
    node_positions: {
      ...document.node_positions,
      [id]: point ? [point[0]! + 200, point[1]!] : [document.nodes.length * 200, 40],
    },
  });
}
export function removeSerialNode(document: EditDocument, id: string) {
  const node = document.nodes.find((n) => n.id === id);
  if (!node || node.inputs.length !== 1 || ['source', 'mask'].includes(node.operation.type))
    throw Error('请通过蒙版面板管理蒙版分支');
  if (document.panel_bindings.some((binding) => binding.node_id === id))
    throw Error('基础节点需保留，可通过眼睛按钮关闭效果');
  const positions = { ...document.node_positions };
  delete positions[id];
  return validateGraph({
    ...document,
    nodes: document.nodes
      .filter((n) => n.id !== id)
      .map((n) => ({
        ...n,
        inputs: n.inputs.map((input) => (input === id ? node.inputs[0]! : input)),
      })),
    output_node: document.output_node === id ? node.inputs[0]! : document.output_node,
    node_positions: positions,
  });
}
export function graphPositions(document: EditDocument) {
  const positions: Record<string, number[]> = {},
    levels = new Map<string, number>(),
    rows = new Map<number, number>();
  const depth = (id: string): number => {
    if (levels.has(id)) return levels.get(id)!;
    levels.set(id, 0);
    const node = document.nodes.find((n) => n.id === id);
    const level = node?.inputs.length ? Math.max(...node.inputs.map(depth)) + 1 : 0;
    levels.set(id, level);
    return level;
  };
  for (const node of document.nodes) {
    const level = depth(node.id),
      row = rows.get(level) ?? 0;
    rows.set(level, row + 1);
    positions[node.id] = document.node_positions[node.id] ?? [30 + level * 200, 35 + row * 100];
  }
  return positions;
}
