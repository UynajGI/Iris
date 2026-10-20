import type { EditDocument } from './editing-client.js';
import { correctors } from './editing-correctors.js';
import { isKeyInput, isKeyOutput, nodeNames, portNames } from './editing-graph.js';
export type NodePort = {
  label: string;
  key: boolean;
  target: string;
  index: number;
  source?: string | undefined;
};
export type ViewNode = {
  id: string;
  name: string;
  kind: string;
  enabled: boolean;
  inputs: NodePort[];
  outputs: { ref: string; key: boolean; aliases?: string[] }[];
};
export const OUTPUT_TERMINAL = '__graph_output';
export function nodeView(document: EditDocument): ViewNode[] {
  const groups = correctors(document),
    hidden = new Set(groups.flatMap((g) => g.members));
  const alpha = document.nodes.find(
    (n) => n.id === document.output_node && n.operation.type === 'set_alpha'
  );
  if (alpha) hidden.add(alpha.id);
  const nodes: ViewNode[] = groups.map((group) => {
    const key = document.nodes.find((n) => n.id === group.key);
    const inputs: NodePort[] = [
      {
        label: 'RGB',
        key: false,
        target: group.anchor ? group.adjustment : group.key ? group.geometry : group.adjustment,
        index: 0,
        source: group.input,
      },
    ];
    inputs.push({
      label: 'Key',
      key: true,
      target: group.key ?? '__key_' + group.id,
      index: 2,
      source: key?.inputs[2] === group.external ? undefined : key?.inputs[2],
    });
    return {
      id: group.id,
      name: group.name,
      kind: 'corrector',
      enabled: document.nodes.find((n) => n.id === group.output)!.enabled,
      inputs,
      outputs: [
        {
          ref: group.output,
          key: false,
          aliases: group.key ? [group.anchor ? group.geometry : group.detail] : [],
        },
        { ref: group.key ?? '__key_' + group.id, key: true },
      ],
    };
  });
  for (const node of document.nodes.filter((n) => !hidden.has(n.id)))
    nodes.push({
      id: node.id,
      name: node.name,
      kind: node.operation.type,
      enabled: node.enabled,
      inputs: portNames(node)
        .map((label, index) => ({
          label,
          key: isKeyInput(node, index),
          target: node.id,
          index,
          source: node.inputs[index],
        }))
        .filter((port) => node.operation.type !== 'parallel_mix' || port.index !== 0),
      outputs: [{ ref: node.id, key: isKeyOutput(node) }],
    });
  nodes.push({
    id: OUTPUT_TERMINAL,
    name: '输出',
    kind: 'terminal',
    enabled: true,
    inputs: [
      {
        label: 'RGB',
        key: false,
        target: OUTPUT_TERMINAL,
        index: 0,
        source: alpha?.inputs[0] ?? document.output_node,
      },
      { label: 'Alpha', key: true, target: OUTPUT_TERMINAL, index: 1, source: alpha?.inputs[1] },
    ],
    outputs: [],
  });
  return nodes;
}
export function viewPositions(document: EditDocument, nodes: ViewNode[]) {
  const owners = new Map(
      nodes.flatMap((n) =>
        n.outputs.flatMap((p) => [p.ref, ...(p.aliases ?? [])].map((ref) => [ref, n.id] as const))
      )
    ),
    depths = new Map<string, number>(),
    rows = new Map<number, number>();
  function depth(id: string): number {
    if (depths.has(id)) return depths.get(id)!;
    depths.set(id, 0);
    const inputs =
      nodes
        .find((n) => n.id === id)
        ?.inputs.map((p) => p.source && owners.get(p.source))
        .filter((s): s is string => !!s) ?? [];
    const level = inputs.length ? Math.max(...inputs.map(depth)) + 1 : 0;
    depths.set(id, level);
    return level;
  }
  return Object.fromEntries(
    nodes.map((n) => {
      const level = depth(n.id),
        row = rows.get(level) ?? 0;
      rows.set(level, row + 1);
      return [n.id, document.node_positions[n.id] ?? [24 + level * 200, 24 + row * 110]];
    })
  );
}
export const viewKind = (node: ViewNode) =>
  node.kind === 'corrector' ? '' : (nodeNames[node.kind as keyof typeof nodeNames] ?? '');
