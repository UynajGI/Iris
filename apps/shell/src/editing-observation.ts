import type { EditDocument } from './editing-client.js';
import { defaultAdjustments } from './editing-document.js';
import { defaultSeparated, type ObservationMode } from './editing-separated.js';
export {
  defaultSeparated,
  separatedParameters,
  withSeparated,
  type ObservationMode,
  type SeparatedAdjustments,
} from './editing-separated.js';

/** View state never enters the editable/saved/exported document or its history. */
export function observationDocument(document: EditDocument, mode: ObservationMode): EditDocument {
  if (mode === 'full') return document;
  let id = 'preview-observation';
  while (document.nodes.some((node) => node.id === id)) id += '-view';
  return {
    ...document,
    nodes: [
      ...document.nodes,
      {
        id,
        name: '观察视图',
        version: 1,
        enabled: true,
        inputs: [document.output_node],
        operation: {
          type: 'adjustments',
          parameters: {
            ...defaultAdjustments(),
            separated: { ...defaultSeparated(), observation: mode },
          },
        },
      },
    ],
    output_node: id,
  };
}
