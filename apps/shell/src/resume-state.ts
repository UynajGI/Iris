import { MAX_COMPARISON_PHOTOS } from './group-session.js';
import type { PhotoFilter } from './types.js';

export interface SelectionResume {
  filter: PhotoFilter;
  focusedId: number | null;
  group?: { groupId: string; focusedId: number | null; comparisonIds: number[] };
}

const object = (value: unknown): Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
const photoId = (value: unknown): number | null =>
  Number.isSafeInteger(value) && Number(value) > 0 ? Number(value) : null;

/** Persist only bounded navigation identifiers, never photos, credentials or media URLs. */
export function validatedResume(
  value: unknown,
  validateFilter: (value: unknown, strict: boolean) => PhotoFilter
): SelectionResume | undefined {
  const input = object(value);
  if (!input.filter) return undefined;
  let filter: PhotoFilter;
  try {
    filter = validateFilter(input.filter, true);
  } catch {
    return undefined;
  }
  const resume: SelectionResume = { filter, focusedId: photoId(input.focusedId) };
  const group = object(input.group);
  if (typeof group.groupId !== 'string' || !group.groupId || group.groupId.length > 128)
    return resume;
  const comparisonIds: number[] = [];
  for (const value of Array.isArray(group.comparisonIds) ? group.comparisonIds : []) {
    const id = photoId(value);
    if (id !== null && !comparisonIds.includes(id)) comparisonIds.push(id);
    if (comparisonIds.length === MAX_COMPARISON_PHOTOS) break;
  }
  resume.group = {
    groupId: group.groupId,
    focusedId: photoId(group.focusedId),
    comparisonIds,
  };
  return resume;
}
