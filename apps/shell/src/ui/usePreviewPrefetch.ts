import { useEffect } from 'react';
import type { IrisClient } from '../client.js';
import type { Photo } from '../types.js';
import { mediaCache, mediaRevision, previewNeighbors, type MediaLease } from '../media-cache.js';

/** Only two compressed neighboring previews, after the active view has had time to load. */
export function usePreviewPrefetch(
  client: IrisClient,
  photos: Photo[],
  focusedId: number | null,
  enabled: boolean
) {
  const targets = enabled ? previewNeighbors(photos, focusedId) : [];
  const identity = JSON.stringify(targets.map((p) => [p.id, mediaRevision(p)]));
  useEffect(() => {
    const leases: MediaLease[] = [];
    const timer = setTimeout(() => {
      const targets: [number, string][] = JSON.parse(identity);
      for (const [id, revision] of targets) {
        const lease = mediaCache(client).acquire(id, 'preview', revision, true);
        leases.push(lease);
        void lease.promise.then(lease.release, lease.release);
      }
    }, 150);
    return () => {
      clearTimeout(timer);
      leases.forEach((lease) => lease.release());
    };
  }, [client, identity]);
}
