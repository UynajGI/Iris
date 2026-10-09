import { useEffect, useState, useSyncExternalStore } from 'react';
import type { IrisStore } from './store.js';
import type { IrisClient } from './client.js';
import type { CommandSystem } from './commands.js';
import { readPhotoAnalysis } from './analysis-readout.js';
import type { UpdaterStore } from './updater.js';

export function useIris(store: IrisStore) { return useSyncExternalStore(store.subscribe, store.getSnapshot, store.getSnapshot); }
export function useUpdater(store: UpdaterStore) { return useSyncExternalStore(store.subscribe, store.getSnapshot, store.getSnapshot); }
export function useGroupSession(store: IrisStore) { return useIris(store).groupSession; }
export function usePhotoAnalysis(store: IrisStore, id: number | null) {
  const state = useIris(store);
  const photo = state.photos.find(photo => photo.id === id) ?? state.groupSession?.photos.find(photo => photo.id === id);
  return photo ? readPhotoAnalysis(photo) : null;
}
export function useCommands(commands: CommandSystem, onError?: (error: unknown) => void): void {
  useEffect(() => commands.attach(window, onError), [commands, onError]);
}
/** Authenticated media is fetched to short-lived object URLs, never token-bearing URLs. */
export function usePhotoUrl(client: IrisClient, id: number | null, kind: 'thumb' | 'preview' | 'original') {
  const [state, setState] = useState<{ url: string | null; error: Error | null }>({ url: null, error: null });
  useEffect(() => {
    const controller = new AbortController();
    let objectUrl: string | undefined;
    setState({ url: null, error: null });
    if (id !== null) void client.media(id, kind, controller.signal).then(blob => {
      if (controller.signal.aborted) return;
      objectUrl = URL.createObjectURL(blob);
      setState({ url: objectUrl, error: null });
    }).catch(error => { if (!controller.signal.aborted) setState({ url: null, error: error instanceof Error ? error : new Error(String(error)) }); });
    return () => { controller.abort(); if (objectUrl) URL.revokeObjectURL(objectUrl); };
  }, [client, id, kind]);
  return state;
}
