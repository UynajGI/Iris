import type { Adjustments, EditDocument } from './editing-client.js';
import type { components } from './generated/api.js';

export type ObservationMode = components['schemas']['ObservationMode'];
export type SeparatedAdjustments = components['schemas']['SeparatedAdjustments'];
export const defaultSeparated = (): SeparatedAdjustments => ({
  tone: { exposure: 0, contrast: 0, highlights: 0, shadows: 0, whites: 0, blacks: 0 },
  color: { temperature: 0, tint: 0, saturation: 0, hue: 0 },
  observation: 'full',
});

export function separatedParameters(a: Adjustments): SeparatedAdjustments {
  const defaults = defaultSeparated();
  return {
    tone: { ...defaults.tone, ...a.separated?.tone },
    color: { ...defaults.color, ...a.separated?.color },
    observation: 'full',
  };
}

/** Observation state is never included in an editable adjustment group. */
export function withSeparated(a: Adjustments, separated: SeparatedAdjustments): Adjustments {
  return { ...a, separated: { ...separated, observation: 'full' } };
}

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
            // The renderer fills omitted legacy fields from its defaults.
            ...neutralAdjustments(),
            separated: { ...defaultSeparated(), observation: mode },
          },
        },
      },
    ],
    output_node: id,
  };
}

// Imported separately to keep the persistent adjustment factories unchanged.
import { defaultAdjustments as neutralAdjustments } from './editing-document.js';
