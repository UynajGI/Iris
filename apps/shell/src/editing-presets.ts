import type { EditDocument, EditRecipe } from './editing-client.js';
import { defaultAdjustments, defaultDetail, panelNode, updatePanel } from './editing-document.js';

/** A traditional preset stores global grading controls only, with no source identity or resources. */
export function colorPresetRecipe(document: EditDocument): EditRecipe {
  const basic = panelNode(document, 'adjustments');
  const detail = panelNode(document, 'detail');
  return structuredClone({
    schema_version: document.schema_version,
    nodes: [
      {
        id: 'source',
        name: '原片',
        version: 1,
        enabled: true,
        inputs: [],
        operation: { type: 'source' },
      },
      {
        id: 'basic',
        name: '影调与颜色',
        version: 1,
        enabled: true,
        inputs: ['source'],
        operation: basic?.enabled
          ? basic.operation
          : { type: 'adjustments', parameters: defaultAdjustments() },
      },
      {
        id: 'detail',
        name: '细节',
        version: 1,
        enabled: true,
        inputs: ['basic'],
        operation: detail?.enabled
          ? detail.operation
          : { type: 'detail', parameters: defaultDetail() },
      },
    ],
    output_node: 'detail',
    panel_bindings: [
      { role: 'basic', node_id: 'basic' },
      { role: 'detail', node_id: 'detail' },
    ],
    node_positions: {},
    raw_white_balance: null,
    output: {
      space: 'srgb',
      tone_map: { kind: 'none' },
      gamut_compression: true,
      background: [1, 1, 1],
    },
  });
}

function hasInput(node: EditRecipe['nodes'][number], id: string) {
  return node.inputs.length === 1 && node.inputs[0] === id;
}

export function colorPresetOperations(recipe: EditRecipe) {
  const source = recipe.nodes.find((node) => node.operation.type === 'source');
  const basic = recipe.nodes.find(
    (node) => node.id === recipe.panel_bindings.find((item) => item.role === 'basic')?.node_id
  );
  const detail = recipe.nodes.find(
    (node) => node.id === recipe.panel_bindings.find((item) => item.role === 'detail')?.node_id
  );
  if (
    recipe.schema_version !== 1 ||
    recipe.nodes.length !== 3 ||
    !source ||
    !source.enabled ||
    source.inputs.length ||
    basic?.operation.type !== 'adjustments' ||
    detail?.operation.type !== 'detail' ||
    !hasInput(basic, source.id) ||
    !hasInput(detail, basic.id) ||
    recipe.output_node !== detail.id
  )
    throw new Error('此预设包含其他节点，暂不能在传统调色面板应用');
  return [
    basic.enabled
      ? basic.operation
      : { type: 'adjustments' as const, parameters: defaultAdjustments() },
    detail.enabled ? detail.operation : { type: 'detail' as const, parameters: defaultDetail() },
  ];
}

/** Reuse the same panel bindings as manual edits; never replace a target's graph or source. */
export function applyColorPreset(document: EditDocument, recipe: EditRecipe): EditDocument {
  let next = document;
  for (const operation of colorPresetOperations(recipe)) {
    next = updatePanel(next, structuredClone(operation));
    const id = panelNode(next, operation.type)!.id;
    next = {
      ...next,
      nodes: next.nodes.map((node) => (node.id === id ? { ...node, enabled: true } : node)),
    };
  }
  return next;
}
