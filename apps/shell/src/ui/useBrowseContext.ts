import { useEffect, useRef } from 'react';
import { BrowseContextPublisher, browseTargets } from '../browse-context.js';
import type { IrisStore, ApplicationState } from '../store.js';

export function useBrowseContext(store: IrisStore, state: ApplicationState, enabled: boolean) {
  const projectId = enabled ? state.project?.id : undefined;
  const publisher = useRef<BrowseContextPublisher | null>(null);
  const group = state.groupSession;
  const targets = browseTargets(
    group?.focusedId ?? state.focusedId,
    group?.comparisonIds ?? [],
    group?.memberIds ?? state.photos.map((photo) => photo.id)
  );
  const key = JSON.stringify(targets);
  useEffect(() => {
    const current =
      projectId === undefined ? null : new BrowseContextPublisher(store.client, projectId);
    publisher.current = current;
    return () => {
      current?.dispose();
      publisher.current = null;
    };
  }, [store, projectId]);
  useEffect(() => {
    publisher.current?.update(JSON.parse(key));
  }, [store, projectId, key]);
}
