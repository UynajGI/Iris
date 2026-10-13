import { useEffect, useMemo, useRef, useState } from 'react';
import { readPhotoAnalysis } from '../analysis-readout.js';
import type { CommandSystem } from '../commands.js';
import { useIris } from '../react.js';
import type { ApplicationState, IrisStore } from '../store.js';
import type { Photo } from '../types.js';
import { faceBoxes } from './photo-readouts.js';
import { filterLabels } from './PhotoFilters.js';
import { useGalleryView } from './useGalleryView.js';
import { useWorkspacePanels } from './useWorkspacePanels.js';
import { useWorkspaceKeyboard } from './workspace-key-handler.js';
import { usePreviewPrefetch } from './usePreviewPrefetch.js';
import { useBrowseContext } from './useBrowseContext.js';

export interface WorkspaceOptions {
  store: IrisStore;
  commands: CommandSystem;
  page: 'projects' | 'photos' | 'settings' | 'help';
  modal: boolean;
  attempt(operation: () => Promise<unknown>): void;
  setError(error: string | null): void;
  setShortcutsOpen(open: boolean): void;
  setScanRoot(root: string | null): void;
}

/** Composition only: selection, panels, gallery and keyboard retain separate owners. */
export function usePhotoWorkspace(options: WorkspaceOptions) {
  const { store, commands, page, modal, attempt, setError, setShortcutsOpen, setScanRoot } =
    options;
  const state = useIris(store);
  useBrowseContext(store, state, page === 'photos');
  const [filterOpen, setFilterOpen] = useState(false);
  const workspace = useRef<HTMLElement>(null);
  const panels = useWorkspacePanels();
  const group = state.groupSession;
  const current = currentPhoto(state);
  const gallery = useGalleryView({ store, state, page, workspace, current });
  const { view } = gallery;
  usePreviewPrefetch(
    store.client,
    group?.photos ?? state.photos,
    current?.id ?? null,
    page === 'photos' && (view === 'loupe' || !!group)
  );
  const progress = state.progress;
  const running = !!progress && ['running', 'paused', 'cancelling'].includes(progress.state);
  const activeFilters = Object.keys(filterLabels).filter(
    (key) => state.filter[key as keyof typeof state.filter] !== undefined
  ).length;
  const counts = useMemo(() => {
    const result = { keep: 0, flag: 0, reject: 0, pending: 0 };
    for (const photo of state.photos) result[photo.decision]++;
    return result;
  }, [state.photos]);
  const faces = useMemo(
    () => (panels.showFaces && view === 'loupe' ? faceBoxes(current) : []),
    [panels.showFaces, view, current]
  );
  const toggleSelected = (id: number, checked: boolean) =>
    store.select(
      checked ? [...state.selectedIds, id] : state.selectedIds.filter((item) => item !== id)
    );
  const clickPhoto = (
    photo: Photo,
    index: number,
    event: { metaKey: boolean; ctrlKey: boolean; shiftKey: boolean }
  ) => {
    if (view === 'grid' && (event.metaKey || event.ctrlKey))
      toggleSelected(photo.id, !state.selectedIds.includes(photo.id));
    else if (view === 'grid' && event.shiftKey && state.focusedId !== null) {
      const from = Math.max(
        0,
        state.photos.findIndex((item) => item.id === state.focusedId)
      );
      const [a, b] = from < index ? [from, index] : [index, from];
      store.select(state.photos.slice(a, b + 1).map((item) => item.id));
    } else {
      store.select([]);
      store.focus(photo.id);
    }
  };
  const model = {
    store,
    state,
    attempt,
    setError,
    setShortcutsOpen,
    setScanRoot,
    filterOpen,
    setFilterOpen,
    workspace,
    ...panels,
    ...gallery,
    ...workspacePositions(state, current),
    group,
    current,
    progress,
    running,
    activeFilters,
    viewing: state.reviewOpen || !!group,
    counts,
    faces,
    hasAnalysis: !!current && readPhotoAnalysis(current).current !== null,
    bulk: !group && state.selectedIds.length > 0,
    toggleSelected,
    clickPhoto,
  };
  useWorkspaceKeyboard(commands, page === 'photos' && !modal && !filterOpen, model);
  useEffect(() => {
    if (page !== 'photos' || !current) return;
    workspace.current
      ?.querySelectorAll(`[data-photo-id="${current.id}"]`)
      .forEach((node) => node.scrollIntoView({ block: 'nearest', inline: 'nearest' }));
  }, [page, current?.id, state.reviewOpen, group?.groupId, panels.stripOpen]);
  return model;
}

function currentPhoto(state: ApplicationState): Photo | undefined {
  const group = state.groupSession;
  return group
    ? group.photos.find((photo) => photo.id === group.focusedId)
    : state.photos.find((photo) => photo.id === state.focusedId);
}
function workspacePositions(state: ApplicationState, current: Photo | undefined) {
  const group = state.groupSession;
  return {
    limit: state.filter.limit ?? 200,
    offset: state.filter.offset ?? 0,
    reviewIndex: current ? state.photos.findIndex((photo) => photo.id === current.id) : -1,
    groupIndex: group ? state.groups.findIndex((item) => item.id === group.groupId) : -1,
    groupPosition: group ? group.memberIds.indexOf(group.focusedId!) : -1,
  };
}
export type PhotoWorkspaceModel = ReturnType<typeof usePhotoWorkspace>;
