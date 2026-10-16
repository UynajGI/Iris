import type { EditDocument, EditRecord, LensProfileSuggestion } from './editing-client.js';
import { defaultGeometry } from './editing-document.js';

const role = 'traditional.lens-profile';
export function lensProfileNode(document: EditDocument | null) {
  const binding = document?.panel_bindings.find((item) => item.role === role);
  return document?.nodes.find(
    (node) => node.id === binding?.node_id && node.operation.type === 'geometry'
  );
}
function validateSuggestion(
  record: EditRecord,
  document: EditDocument,
  suggestion: LensProfileSuggestion
) {
  if (
    record.id !== suggestion.document_id ||
    record.revision !== suggestion.expected_revision ||
    document.source.fingerprint !== suggestion.source_fingerprint
  )
    throw new Error('照片或调色版本已变更，请重新匹配镜头');
  if (!suggestion.matching.applicable || !suggestion.matching.correction)
    throw new Error('镜头配置与拍摄信息不匹配');
  if (suggestion.placement !== 'optical_coefficients_only_on_uncropped_source_extent')
    throw new Error('镜头配置的坐标定义不受支持');
}
function validatePlacement(existing: EditDocument['nodes'][number] | undefined, sourceId: string) {
  if (!existing) return;
  if (existing.operation.type !== 'geometry') throw new Error('镜头配置节点类型错误');
  const parameters = existing.operation.parameters;
  if (
    existing.inputs.length !== 1 ||
    existing.inputs[0] !== sourceId ||
    JSON.stringify(parameters.crop) !== '[0,0,1,1]' ||
    parameters.rotation_degrees !== 0 ||
    JSON.stringify(parameters.perspective) !== '[1,0,0,0,1,0,0,0,1]'
  )
    throw new Error('镜头校正节点已由节点视图修改，请检查其位置');
}
/** Optical calibration runs at the uncropped source, before user geometry and local edits. */
export function applyLensProfile(
  record: EditRecord,
  document: EditDocument,
  suggestion: LensProfileSuggestion
): EditDocument {
  validateSuggestion(record, document, suggestion);
  const source = document.nodes.find((node) => node.operation.type === 'source');
  if (!source) throw new Error('编辑文档缺少原片节点');
  const existing = lensProfileNode(document);
  validatePlacement(existing, source.id);
  let id = existing?.id ?? 'lens-profile';
  for (let index = 1; !existing && document.nodes.some((node) => node.id === id); index++)
    id = `lens-profile-${index}`;
  const node: EditDocument['nodes'][number] = {
    id,
    name: '镜头配置校正',
    version: 1,
    enabled: true,
    inputs: [source.id],
    operation: {
      type: 'geometry',
      parameters: { ...defaultGeometry(), ...suggestion.matching.correction },
    },
  };
  return {
    ...document,
    nodes: existing
      ? document.nodes.map((item) => (item.id === id ? node : item))
      : [
          ...document.nodes.map((item) => ({
            ...item,
            inputs: item.inputs.map((input) => (input === source.id ? id : input)),
          })),
          node,
        ],
    output_node: document.output_node === source.id ? id : document.output_node,
    panel_bindings: existing
      ? document.panel_bindings
      : [...document.panel_bindings, { role, node_id: id }],
  };
}

export type InstalledLensProfile = { hash: string; name: string };
const storageKey = 'iris:lens-profiles:v1';
export function installedLensProfiles(): InstalledLensProfile[] {
  try {
    const items: unknown = JSON.parse(localStorage.getItem(storageKey) ?? '[]');
    if (!Array.isArray(items)) return [];
    return items
      .filter(
        (item): item is InstalledLensProfile =>
          !!item &&
          typeof item === 'object' &&
          typeof item.hash === 'string' &&
          /^[a-f0-9]{64}$/.test(item.hash) &&
          typeof item.name === 'string' &&
          item.name.length <= 160
      )
      .slice(0, 32);
  } catch {
    return [];
  }
}
export function rememberLensProfiles(items: InstalledLensProfile[]) {
  try {
    localStorage.setItem(storageKey, JSON.stringify(items.slice(0, 32)));
  } catch {
    /* Current session remains usable. */
  }
}
