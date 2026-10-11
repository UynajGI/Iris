import { useEffect, useRef } from 'react';
import type { CommandSystem } from '../commands.js';
import type { PhotoWorkspaceModel } from './usePhotoWorkspace.js';
import { workspaceShortcut } from './workspace-shortcuts.js';

/** Commands attach first so native editing and Shift+Z arbitration remain intact. */
export function useWorkspaceKeyboard(
  commands: CommandSystem,
  active: boolean,
  model: PhotoWorkspaceModel
) {
  const latest = useRef(model);
  latest.current = model;
  useEffect(() => {
    if (!active) return;
    return commands.attach(window, (reason) => latest.current.setError(String(reason)));
  }, [commands, active]);
  useEffect(() => {
    if (!active) return;
    const handler = (event: KeyboardEvent) => handleWorkspaceKey(event, latest.current);
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [active]);
}

export function handleWorkspaceKey(event: KeyboardEvent, model: PhotoWorkspaceModel): void {
  const action = workspaceShortcut(event);
  if (action === null) return;
  const take = (operation: () => void) => {
    event.preventDefault();
    operation();
  };
  if (typeof action === 'number') {
    if (model.current && !model.group?.loading)
      take(() => model.attempt(() => model.store.mark({ rating: action })));
    return;
  }
  const handlers: Record<Exclude<typeof action, number>, () => void> = {
    panels: () => take(() => togglePanels(model)),
    help: () => take(() => model.setShortcutsOpen(true)),
    grid: () => take(() => model.setView('grid')),
    loupe: () => {
      if (model.current && !model.group && !model.state.reviewOpen)
        take(() => model.setView('loupe'));
    },
    faces: () => {
      if (model.view === 'loupe' && model.hasAnalysis)
        take(() => model.setShowFaces((value) => !value));
    },
    fullscreen: () => {
      if (model.viewing && model.fullscreen.supported)
        take(() => {
          void model.fullscreen.toggle().catch((reason) => model.setError(String(reason)));
        });
    },
    zoomIn: () => zoomShortcut(model, () => take(() => model.stepZoom(1.25))),
    zoomOut: () => zoomShortcut(model, () => take(() => model.stepZoom(0.8))),
    zoomToggle: () =>
      zoomShortcut(model, () => {
        if (model.zoomApi.current)
          take(() =>
            model.zoomReadout.fitted
              ? model.zoomApi.current?.actual()
              : model.zoomApi.current?.fit()
          );
      }),
  };
  handlers[action]();
}

function togglePanels(model: PhotoWorkspaceModel): void {
  const open = model.libraryOpen || model.rightOpen;
  if (model.wide) model.setLeftOpen(!open);
  else model.setLibraryDrawer(false);
  model.setRightOpen(!open);
}
function zoomShortcut(model: PhotoWorkspaceModel, operation: () => void): void {
  if (!model.group && model.current) operation();
}
