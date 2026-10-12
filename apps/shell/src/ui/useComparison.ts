import { useEffect, useState } from 'react';
import type { IrisClient } from '../client.js';
import type { ComparisonReport, Photo } from '../types.js';
import { mediaRevision } from '../media-cache.js';

export function useComparison(
  client: IrisClient,
  projectId: number,
  photos: Photo[],
  referenceId: number,
  enabled: boolean
) {
  const ids = photos.map((photo) => photo.id);
  const key = JSON.stringify([
    projectId,
    ids,
    referenceId,
    enabled,
    photos.map((photo) => [mediaRevision(photo), photo.analysis_status]),
  ]);
  const [response, setResponse] = useState<{
    key: string;
    report?: ComparisonReport;
    error?: string;
  } | null>(null);
  useEffect(() => {
    if (!enabled || ids.length < 2) return;
    const controller = new AbortController();
    void client
      .compare(projectId, { photo_ids: ids, reference_photo_id: referenceId }, controller.signal)
      .then((report) => {
        if (!controller.signal.aborted) setResponse({ key, report });
      })
      .catch((cause) => {
        if (!controller.signal.aborted) setResponse({ key, error: String(cause) });
      });
    return () => controller.abort();
  }, [client, key]);
  return response?.key === key ? response : null;
}
