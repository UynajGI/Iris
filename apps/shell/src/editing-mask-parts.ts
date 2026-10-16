import {
  createMaskDefinition,
  validateMaskBudget,
  type BrushStroke,
  type MaskDefinition,
  type MaskKind,
} from './editing-masks.js';

export type MaskCombine = Extract<MaskDefinition, { kind: 'combine' }>['operation'];
export const combineNames: Record<MaskCombine, string> = {
  add: '添加',
  subtract: '减去',
  intersect: '相交',
};
export type MaskPart = {
  path: string;
  definition: Extract<MaskDefinition, { kind: MaskKind }>;
  relation: string;
};

/** Paths address the saved expression, so editing one part never flattens its neighbours. */
export function maskParts(mask: MaskDefinition, path = '', relation = '基础'): MaskPart[] {
  if (mask.kind === 'combine')
    return [
      ...maskParts(mask.left, path + 'L', relation),
      ...maskParts(
        mask.right,
        path + 'R',
        relation === '基础'
          ? combineNames[mask.operation]
          : `${relation} / ${combineNames[mask.operation]}`
      ),
    ];
  if (mask.kind === 'invert' || mask.kind === 'opacity')
    return maskParts(
      mask.mask,
      path + 'M',
      mask.kind === 'invert' ? `${relation}（反选）` : relation
    );
  return [{ path, definition: mask, relation }];
}

function editPart(
  mask: MaskDefinition,
  path: string,
  replacement: MaskDefinition | null
): MaskDefinition | null {
  if (!path) return replacement;
  const next = path.slice(1);
  if (path[0] === 'M' && (mask.kind === 'invert' || mask.kind === 'opacity')) {
    const child = editPart(mask.mask, next, replacement);
    return child ? { ...mask, mask: child } : null;
  }
  if (mask.kind === 'combine' && (path[0] === 'L' || path[0] === 'R')) {
    const left = path[0] === 'L' ? editPart(mask.left, next, replacement) : mask.left;
    const right = path[0] === 'R' ? editPart(mask.right, next, replacement) : mask.right;
    return left && right ? { ...mask, left, right } : (left ?? right);
  }
  throw new Error('蒙版组成已变化，请重新选择');
}

export function replaceMaskPart(
  mask: MaskDefinition,
  path: string,
  replacement: MaskDefinition
): MaskDefinition {
  return editPart(mask, path, replacement)!;
}

export function maskPartAt(mask: MaskDefinition, index: number): MaskPart {
  const parts = maskParts(mask);
  return parts[Math.max(0, Math.min(index, parts.length - 1))]!;
}

export function paintMaskPart(
  mask: MaskDefinition,
  part: MaskPart,
  stroke: BrushStroke,
  erase: boolean
): MaskDefinition {
  if (part.definition.kind !== 'brush') return mask;
  const replacement: MaskDefinition = erase
    ? {
        kind: 'combine',
        operation: 'subtract',
        left: part.definition,
        right: { kind: 'brush', strokes: [stroke] },
      }
    : { kind: 'brush', strokes: [...part.definition.strokes, stroke] };
  return replaceMaskPart(mask, part.path, replacement);
}

export function removeMaskPart(mask: MaskDefinition, path: string): MaskDefinition {
  const result = editPart(mask, path, null);
  if (!result) throw new Error('最后一个组成部分不能移除，请删除整个蒙版');
  return result;
}

export function appendMaskPart(
  mask: MaskDefinition,
  operation: MaskCombine,
  kind: MaskKind
): MaskDefinition {
  const result: MaskDefinition = {
    kind: 'combine',
    operation,
    left: mask,
    right: createMaskDefinition(kind),
  };
  validateMaskBudget(result);
  return result;
}
