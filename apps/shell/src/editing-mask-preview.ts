import type { EditDocument } from './editing-client.js';
import type { MaskDefinition } from './editing-masks.js';

export function needsWorkingMask(mask: MaskDefinition): boolean {
  if (mask.kind === 'luminance_range' || mask.kind === 'color_range') return true;
  if (mask.kind === 'invert' || mask.kind === 'opacity') return needsWorkingMask(mask.mask);
  return mask.kind === 'combine' && (needsWorkingMask(mask.left) || needsWorkingMask(mask.right));
}

/** Conservative display-grid identity; equal dimensions alone do not prove alignment. */
export function maskGridKey(document: EditDocument): string {
  return JSON.stringify([
    document.source,
    document.output_node,
    document.nodes.filter((node) => node.operation.type === 'geometry'),
  ]);
}

export function decodeMaskPreview(
  response: { blob: Blob; headers: Headers },
  revision: number,
  sequence: number
): Blob {
  const { headers, blob } = response;
  if (
    headers.get('x-iris-revision') !== String(revision) ||
    headers.get('x-iris-sequence') !== String(sequence) ||
    headers.get('x-iris-mask-domain') !== 'linear_rec2020_d65' ||
    headers.get('x-iris-approximate') !== 'true' ||
    headers.get('content-type')?.split(';')[0] !== 'image/png'
  )
    throw new Error('蒙版覆盖响应无效或已过期');
  return blob;
}
