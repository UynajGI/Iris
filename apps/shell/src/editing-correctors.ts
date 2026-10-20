import type { EditDocument } from './editing-client.js';
import {
  defaultAdjustments,
  defaultDetail,
  defaultGeometry,
  panelNode,
} from './editing-document.js';
import { validateGraph, type EditNode } from './editing-graph.js';
export type Corrector = {
  id: string;
  name: string;
  input: string;
  output: string;
  adjustment: string;
  detail: string;
  geometry: string;
  key?: string | undefined;
  mask?: string | undefined;
  external?: string | undefined;
  anchor?: string | undefined;
  members: string[];
};
const role = (id: string, part: string) => `corrector:${id}:${part}`;
export const fullKey = () => ({
  kind: 'invert' as const,
  mask: { kind: 'brush' as const, strokes: [] },
});
export function makeNode(
  id: string,
  inputs: string[],
  operation: EditNode['operation'],
  name = '节点'
): EditNode {
  return { id, name, version: 1, enabled: true, inputs, operation };
}
function managedCorrectors(document: EditDocument): Corrector[] {
  const result: Corrector[] = [];
  const a = panelNode(document, 'adjustments'),
    d = panelNode(document, 'detail'),
    g = panelNode(document, 'geometry');
  if (document.panel_bindings.some((b) => b.role.startsWith('corrector:') && b.node_id === a?.id))
    return [];
  if (a && d && g && d.inputs[0] === a.id && g.inputs[0] === d.id) {
    const members = [a.id, d.id, g.id];
    const exposed = document.nodes.some(
      (n) => !members.includes(n.id) && n.inputs.some((i) => i === a.id || i === d.id)
    );
    if (!exposed)
      result.push({
        id: g.id,
        name: g.name === '几何与光学' ? '基础调整' : g.name,
        input: a.inputs[0]!,
        output: g.id,
        adjustment: a.id,
        detail: d.id,
        geometry: g.id,
        members,
      });
  }
  return result;
}
function correctorInput(
  anchor: EditNode | undefined,
  key: EditNode | undefined,
  geometry: EditNode,
  adjustment: EditNode
) {
  return anchor?.inputs[0] ?? (key ? geometry.inputs[0]! : adjustment.inputs[0]!);
}
export function correctors(document: EditDocument): Corrector[] {
  const result = managedCorrectors(document);
  const groups = new Set(
    document.panel_bindings
      .filter((b) => b.role.startsWith('corrector:'))
      .map((b) => b.role.split(':')[1]!)
  );
  for (const id of groups) {
    const part = (kind: string) =>
      document.nodes.find(
        (n) => n.id === document.panel_bindings.find((b) => b.role === role(id, kind))?.node_id
      );
    const a = part('adjustments'),
      d = part('detail'),
      g = part('geometry'),
      output = part('output') ?? g;
    if (!a || !d || !g || !output) continue;
    const members = document.panel_bindings
      .filter((b) => b.role.startsWith(`corrector:${id}:`))
      .map((b) => b.node_id);
    result.push({
      id: output.id,
      name: output.name,
      input: correctorInput(part('anchor'), part('key'), g, a),
      output: output.id,
      adjustment: a.id,
      detail: d.id,
      geometry: g.id,
      key: part('key')?.id,
      mask: part('mask')?.id,
      external: part('external')?.id,
      anchor: part('anchor')?.id,
      members,
    });
  }
  const order = new Map(document.nodes.map((node, index) => [node.id, index]));
  return result.sort((a, b) => order.get(a.adjustment)! - order.get(b.adjustment)!);
}
export function correctorDocument(document: EditDocument, group: Corrector): EditDocument {
  return {
    ...document,
    panel_bindings: [
      ...document.panel_bindings.filter(
        (b) =>
          ![
            'basic',
            'detail',
            'geometry',
            'traditional.adjustments',
            'traditional.detail',
            'traditional.geometry',
          ].includes(b.role)
      ),
      { role: 'basic', node_id: group.adjustment },
      { role: 'detail', node_id: group.detail },
      { role: 'geometry', node_id: group.geometry },
    ],
  };
}
export function updateCorrector(
  document: EditDocument,
  group: Corrector,
  edited: EditDocument
): EditDocument {
  return validateGraph({
    ...document,
    nodes: document.nodes.map((node) =>
      node.id === group.anchor
        ? {
            ...node,
            operation: structuredClone(
              edited.nodes.find((n) => n.id === group.geometry)!.operation
            ),
          }
        : group.members.includes(node.id)
          ? (edited.nodes.find((n) => n.id === node.id) ?? node)
          : node
    ),
  });
}
/** Upgrade a legacy chain only when the user adds a key or a branch; full-key output is identical. */
export function completeCorrector(
  document: EditDocument,
  group: Corrector
): { document: EditDocument; group: Corrector } {
  if (group.key) return { document, group };
  const identifier = crypto.randomUUID(),
    anchor = identifier + '-anchor',
    mask = identifier + '-mask',
    external = identifier + '-ext',
    key = identifier + '-key',
    output = identifier + '-out';
  const geometry = document.nodes.find((n) => n.id === group.geometry)!;
  const parts = {
    adjustments: group.adjustment,
    detail: group.detail,
    geometry: group.geometry,
    anchor,
    mask,
    external,
    key,
    output,
  };
  const next = validateGraph({
    ...document,
    nodes: [
      ...document.nodes.map((n) =>
        group.members.includes(n.id)
          ? n
          : { ...n, inputs: n.inputs.map((i) => (i === group.output ? output : i)) }
      ),
      {
        ...makeNode(anchor, [group.input], structuredClone(geometry.operation)),
        enabled: geometry.enabled,
      },
      makeNode(mask, [anchor], { type: 'mask', parameters: fullKey() }),
      makeNode(external, [anchor], { type: 'mask', parameters: fullKey() }),
      makeNode(key, [anchor, mask, external], {
        type: 'key_mix',
        parameters: { gain: 1, invert: false },
      }),
      makeNode(
        output,
        [anchor, group.geometry, key],
        { type: 'blend', parameters: { opacity: 1 } },
        group.name
      ),
    ],
    output_node: document.output_node === group.output ? output : document.output_node,
    panel_bindings: [
      ...document.panel_bindings.filter(
        (b) => !(b.role.startsWith('corrector:') && group.members.includes(b.node_id))
      ),
      ...Object.entries(parts).map(([part, node_id]) => ({
        role: role(identifier, part),
        node_id,
      })),
    ],
    node_positions: document.node_positions[group.id]
      ? { ...document.node_positions, [output]: document.node_positions[group.id]! }
      : document.node_positions,
  });
  return { document: next, group: correctors(next).find((g) => g.id === output)! };
}
export function addCorrector(
  document: EditDocument,
  parent: string,
  identifier: string = crypto.randomUUID(),
  splice = true
): EditDocument {
  const source = document.nodes.find((n) => n.id === parent);
  if (!source || ['mask', 'key_mix', 'alpha_mask'].includes(source.operation.type))
    throw Error('请选择图像节点');
  const ids = Object.fromEntries(
    ['adjustments', 'detail', 'geometry', 'mask', 'external', 'key', 'output'].map(
      (kind, index) => [kind, `${identifier}-${index}`]
    )
  );
  const a = ids.adjustments!,
    d = ids.detail!,
    g = ids.geometry!,
    m = ids.mask!,
    e = ids.external!,
    k = ids.key!,
    b = ids.output!;
  // Sizing precedes correction and key, so RGB and Key share one coordinate grid.
  const nodes = [
    makeNode(g, [parent], { type: 'geometry', parameters: defaultGeometry() }),
    makeNode(a, [g], { type: 'adjustments', parameters: defaultAdjustments() }),
    makeNode(d, [a], { type: 'detail', parameters: defaultDetail() }),
    makeNode(m, [g], { type: 'mask', parameters: fullKey() }),
    makeNode(e, [g], { type: 'mask', parameters: fullKey() }),
    makeNode(k, [g, m, e], { type: 'key_mix', parameters: { gain: 1, invert: false } }),
    makeNode(b, [g, d, k], { type: 'blend', parameters: { opacity: 1 } }),
  ];
  return validateGraph({
    ...document,
    nodes: [
      ...document.nodes.map((n) =>
        splice ? { ...n, inputs: n.inputs.map((i) => (i === parent ? b : i)) } : n
      ),
      ...nodes,
    ],
    output_node: splice && document.output_node === parent ? b : document.output_node,
    panel_bindings: [
      ...document.panel_bindings,
      ...Object.entries(ids).map(([part, node_id]) => ({ role: role(identifier, part), node_id })),
    ],
  });
}
export function removeCorrector(document: EditDocument, group: Corrector): EditDocument {
  if (
    document.panel_bindings.some(
      (b) => ['basic', 'detail', 'geometry'].includes(b.role) && group.members.includes(b.node_id)
    )
  )
    throw Error('基础调整保留，可关闭效果');
  const reduced = collapseBranchMixers(document, group);
  const external = reduced.nodes.filter((n) => !group.members.includes(n.id));
  if (external.some((n) => n.inputs.some((i) => group.members.includes(i) && i !== group.output)))
    throw Error('节点的 Key 仍被使用，请先断开连接');
  return validateGraph({
    ...reduced,
    nodes: external.map((n) => ({
      ...n,
      inputs: n.inputs.map((i) => (i === group.output ? group.input : i)),
    })),
    output_node: reduced.output_node === group.output ? group.input : reduced.output_node,
    panel_bindings: document.panel_bindings.filter((b) => !group.members.includes(b.node_id)),
    node_positions: Object.fromEntries(
      Object.entries(reduced.node_positions).filter(([id]) => !group.members.includes(id))
    ),
  });
}
function collapseBranchMixers(document: EditDocument, group: Corrector): EditDocument {
  const aliases = new Map<string, string>();
  const raw = group.anchor ? group.geometry : group.detail;
  const groups = correctors(document);
  for (const node of document.nodes) {
    if (
      node.operation.type === 'layer_mix' &&
      node.inputs[1] === raw &&
      node.inputs[2] === group.key
    )
      aliases.set(node.id, node.inputs[0]!);
    if (node.operation.type !== 'parallel_mix') continue;
    const branch = node.inputs[1] === raw ? 1 : node.inputs[2] === raw ? 2 : 0;
    if (!branch || node.inputs[branch + 2] !== group.key) continue;
    const other = branch === 1 ? 2 : 1;
    const sibling = groups.find(
      (g) =>
        (g.anchor ? g.geometry : g.detail) === node.inputs[other] &&
        g.key === node.inputs[other + 2]
    );
    if (sibling) aliases.set(node.id, sibling.output);
  }
  const resolve = (id: string): string => (aliases.has(id) ? resolve(aliases.get(id)!) : id);
  return {
    ...document,
    nodes: document.nodes
      .filter((n) => !aliases.has(n.id))
      .map((n) => ({ ...n, inputs: n.inputs.map(resolve) })),
    output_node: resolve(document.output_node),
    node_positions: Object.fromEntries(
      Object.entries(document.node_positions).filter(([id]) => !aliases.has(id))
    ),
  };
}
export function visualGraph(document: EditDocument) {
  const groups = correctors(document),
    hidden = new Set(groups.flatMap((g) => g.members.filter((id) => id !== g.id)));
  return {
    ...document,
    nodes: document.nodes
      .filter((n) => !hidden.has(n.id))
      .map((n) => {
        const group = groups.find((g) => g.id === n.id);
        return group ? { ...n, name: group.name, inputs: [group.input] } : n;
      }),
  };
}
export function connectCorrector(
  document: EditDocument,
  source: string,
  group: Corrector,
  key: boolean
) {
  const target = key
    ? group.key
    : group.anchor
      ? group.adjustment
      : group.key
        ? group.geometry
        : group.adjustment;
  if (!target) throw Error('旧版基础调整没有外部 Key 入口；请在其后添加节点');
  return validateGraph({
    ...document,
    nodes: document.nodes.map((n) =>
      n.id === target || (!key && n.id === group.anchor)
        ? { ...n, inputs: n.inputs.map((id, index) => (index === (key ? 2 : 0) ? source : id)) }
        : n
    ),
  });
}
export function addAlphaOutput(document: EditDocument, key: string) {
  const existing = document.nodes.find(
    (n) => n.operation.type === 'set_alpha' && n.id === document.output_node
  );
  const id = existing?.id ?? crypto.randomUUID();
  return validateGraph({
    ...document,
    nodes: existing
      ? document.nodes.map((n) => (n.id === id ? { ...n, inputs: [n.inputs[0]!, key] } : n))
      : [
          ...document.nodes,
          makeNode(id, [document.output_node, key], { type: 'set_alpha' }, 'Alpha 输出'),
        ],
    output_node: id,
  });
}
export function setRgbOutput(document: EditDocument, image: string) {
  const alpha = document.nodes.find(
    (n) => n.id === document.output_node && n.operation.type === 'set_alpha'
  );
  return validateGraph(
    alpha
      ? {
          ...document,
          nodes: document.nodes.map((n) =>
            n.id === alpha.id ? { ...n, inputs: [image, n.inputs[1]!] } : n
          ),
        }
      : { ...document, output_node: image }
  );
}
export function disconnectKey(document: EditDocument, target: string, index: number): EditDocument {
  const group = correctors(document).find((g) => g.key === target);
  if (group?.external)
    return validateGraph({
      ...document,
      nodes: document.nodes.map((n) =>
        n.id === target
          ? { ...n, inputs: n.inputs.map((input, i) => (i === index ? group.external! : input)) }
          : n
      ),
    });
  const alpha = document.nodes.find(
    (n) => n.id === document.output_node && n.operation.type === 'set_alpha'
  );
  if (target === '__graph_output' && alpha) {
    const used = document.nodes.some((n) => n.inputs.includes(alpha.id));
    return validateGraph({
      ...document,
      output_node: alpha.inputs[0]!,
      nodes: used ? document.nodes : document.nodes.filter((n) => n.id !== alpha.id),
      node_positions: Object.fromEntries(
        Object.entries(document.node_positions).filter(([id]) => id !== alpha.id)
      ),
    });
  }
  throw Error('此端口需要有效输入，请连接另一个 Key');
}
export function addMixerBranch(document: EditDocument, group: Corrector, layer: boolean) {
  ({ document, group } = completeCorrector(document, group));
  let next = addCorrector(document, group.input, crypto.randomUUID(), false);
  const branch = correctors(next).at(-1)!;
  const geometry = document.nodes.find((n) => n.id === group.geometry)!;
  next = {
    ...next,
    nodes: next.nodes.map((n) =>
      n.id === branch.geometry ? { ...n, operation: structuredClone(geometry.operation) } : n
    ),
  };
  const id = crypto.randomUUID(),
    key = branch.key!;
  const mix = makeNode(
    id,
    layer
      ? [group.output, branch.detail, key]
      : [
          group.anchor ?? group.geometry,
          group.anchor ? group.geometry : group.detail,
          branch.detail,
          group.key!,
          key,
        ],
    { type: layer ? 'layer_mix' : 'parallel_mix' },
    layer ? '图层混合' : '并行混合'
  );
  next = {
    ...next,
    nodes: [
      ...next.nodes.map((n) =>
        !branch.members.includes(n.id)
          ? { ...n, inputs: n.inputs.map((i) => (i === group.output ? id : i)) }
          : n
      ),
      mix,
    ],
    output_node: document.output_node === group.output ? id : document.output_node,
  };
  return validateGraph(next);
}
