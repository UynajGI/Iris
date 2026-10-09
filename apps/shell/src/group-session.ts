import type { IrisClient } from './client.js';
import type { Photo } from './types.js';

/** Ordered server group membership, independent of the library's current page. */
export interface GroupSessionState {
  groupId: string;
  memberIds: number[];
  photos: Photo[];
  focusedId: number | null;
  comparisonIds: number[];
  loading: boolean;
}

export async function loadGroupMembers(client: IrisClient, projectId: number, ids: number[], current: () => boolean): Promise<Photo[]> {
  const photos = new Array<Photo>(ids.length);
  let next = 0;
  await Promise.all(Array.from({ length: Math.min(6, ids.length) }, async () => {
    while (current()) {
      const index = next++;
      if (index >= ids.length) return;
      const photo = await client.photo(ids[index]!);
      if (photo.id !== ids[index] || photo.project_id !== projectId || photo.missing || photo.quarantined) {
        throw new Error('Group member is unavailable or belongs to another project');
      }
      photos[index] = photo;
    }
  }));
  return photos;
}
