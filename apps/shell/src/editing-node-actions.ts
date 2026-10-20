import type { EditDocument } from './editing-client.js';
import {
  addAlphaOutput,
  completeCorrector,
  connectCorrector,
  correctors,
  correctorDocument,
  setRgbOutput,
  type Corrector,
} from './editing-correctors.js';
import { connectNode, validateGraph } from './editing-graph.js';
import { OUTPUT_TERMINAL, type NodePort } from './editing-node-view.js';

export const virtualKey = (id: string) => `__key_${id}`;
export function nodeCommand(key: string, alt: boolean, control: boolean) {
  const lower = key.toLowerCase();
  if (alt) return ({ s: 'serial', p: 'parallel', l: 'layer' } as const)[lower as 's' | 'p' | 'l'];
  if (control && lower === 'd') return 'bypass';
  if (lower === 'delete' || lower === 'backspace') return 'delete';
  return undefined;
}
export function selectedCorrectorNode(document: EditDocument | null, id: string | null) {
  if (!document) return undefined;
  let node =
    document.nodes.find((n) => n.id === id) ??
    document.nodes.find((n) => n.id === document.output_node);
  if (node?.operation.type === 'set_alpha')
    node = document.nodes.find((n) => n.id === node!.inputs[0]);
  const group = correctors(document).find((g) => g.members.includes(node?.id ?? ''));
  return group ? document.nodes.find((n) => n.id === group.output) : node;
}
export function nodePanelDocument(document: EditDocument, id: string | null) {
  if (id === null) return document;
  const node = selectedCorrectorNode(document, id);
  const group = correctors(document).find((g) => g.id === node?.id);
  return group ? correctorDocument(document, group) : document;
}
export function nodeTarget(enabled: boolean, selected: string | null) {
  return enabled ? (selected ?? '__selected_output') : null;
}
export function nodeEditingContext(
  document: EditDocument | null,
  enabled: boolean,
  selected: string | null
) {
  const activeNode = selectedCorrectorNode(document, selected);
  const group =
    enabled && document ? correctors(document).find((g) => g.id === activeNode?.id) : undefined;
  const scopedDocument =
    document && nodePanelDocument(document, enabled ? (activeNode?.id ?? null) : null);
  return { activeNode, scopedGroup: group, scopedDocument };
}

/** Resolve display ports atomically. Merely opening an old document never rewrites it. */
export function connectVisiblePort(
  document: EditDocument,
  source: { ref: string; key: boolean },
  port: NodePort
) {
  if (source.key !== port.key) throw Error('图像与蒙版端口不能混接');
  const originalGroups = correctors(document);
  const targetGroup = originalGroups.find((g) =>
    [g.geometry, g.adjustment, g.key, virtualKey(g.id)].includes(port.target)
  );
  const sourceGroup = originalGroups.find((g) =>
    [g.output, g.key, virtualKey(g.id)].includes(source.ref)
  );
  let next = document,
    ref = source.ref;
  if (sourceGroup && source.key) {
    const ready = completeCorrector(next, sourceGroup);
    next = ready.document;
    ref = ready.group.key!;
  }
  if (port.target === OUTPUT_TERMINAL)
    return source.key ? addAlphaOutput(next, ref) : setRgbOutput(next, ref);
  if (targetGroup) {
    let group = correctors(next).find((g) => g.adjustment === targetGroup.adjustment)!;
    if (port.key) ({ document: next, group } = completeCorrector(next, group));
    return connectCorrector(next, ref, group, port.key);
  }
  const target = next.nodes.find((n) => n.id === port.target);
  const unmixed =
    sourceGroup?.key &&
    ((target?.operation.type === 'parallel_mix' && [1, 2].includes(port.index)) ||
      (target?.operation.type === 'layer_mix' && port.index === 1));
  return connectNode(
    next,
    unmixed ? (sourceGroup.anchor ? sourceGroup.geometry : sourceGroup.detail) : ref,
    port.target,
    port.index
  );
}

export function setCorrectorEnabled(document: EditDocument, group: Corrector, enabled: boolean) {
  const ready = completeCorrector(document, group);
  // Keep sizing and the matte coordinate grid stable; bypass the correction at the group output.
  return validateGraph({
    ...ready.document,
    nodes: ready.document.nodes.map((n) => (n.id === ready.group.output ? { ...n, enabled } : n)),
  });
}

export function editCorrectorMask(document: EditDocument, group: Corrector) {
  const ready = completeCorrector(document, group),
    g = ready.group;
  if (ready.document.panel_bindings.some((b) => b.role === 'local-adjustments:' + g.mask))
    return { document: ready.document, mask: g.mask! };
  return {
    document: validateGraph({
      ...ready.document,
      nodes: ready.document.nodes.map((n) =>
        n.id === g.mask
          ? {
              ...n,
              name: g.name + '选区',
              operation: {
                type: 'mask' as const,
                parameters: {
                  kind: 'linear_gradient' as const,
                  start: [0.5, 0.25],
                  end: [0.5, 0.75],
                },
              },
            }
          : n
      ),
      panel_bindings: [
        ...ready.document.panel_bindings,
        { role: 'local-adjustments:' + g.mask, node_id: g.adjustment },
        { role: 'local-detail:' + g.mask, node_id: g.detail },
      ],
    }),
    mask: g.mask!,
  };
}

export function updateCorrectorKey(
  document: EditDocument,
  group: Corrector,
  gain: number,
  invert: boolean
) {
  const ready = completeCorrector(document, group);
  return validateGraph({
    ...ready.document,
    nodes: ready.document.nodes.map((n) =>
      n.id === ready.group.key
        ? { ...n, operation: { type: 'key_mix' as const, parameters: { gain, invert } } }
        : n
    ),
  });
}
