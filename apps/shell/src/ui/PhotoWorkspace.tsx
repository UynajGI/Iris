import { CompareCanvas } from './CompareCanvas.js';
import { FilmstripDock } from './FilmstripDock.js';
import { GridGallery } from './GridGallery.js';
import { LoupeCanvas } from './LoupeCanvas.js';
import type { PhotoWorkspaceModel } from './usePhotoWorkspace.js';
import { InspectorPanel, LibraryPanel } from './WorkspacePanels.js';
import { EmptyCanvas, WorkspaceStatus } from './WorkspaceStatus.js';
import { GroupNavigation, WorkspaceToolbar } from './WorkspaceToolbar.js';

export function PhotoWorkspace({ model }: { model: PhotoWorkspaceModel }) {
  const {
    workspace,
    libraryOpen,
    rightOpen,
    wide,
    libraryDrawer,
    setLibraryDrawer,
    group,
    view,
    current,
    state,
  } = model;
  return (
    <main
      ref={workspace}
      className={`workspace ${libraryOpen ? '' : 'no-left'} ${rightOpen ? '' : 'no-right'}`}
    >
      {!wide && libraryDrawer && (
        <button
          type="button"
          className="drawer-scrim"
          aria-label="关闭照片库面板"
          onClick={() => setLibraryDrawer(false)}
        />
      )}
      <LibraryPanel model={model} />

      <section className="center" aria-label="照片工作区">
        <WorkspaceToolbar model={model} />

        <GroupNavigation model={model} />

        <div className="canvas">
          {group ? (
            <CompareCanvas model={model} />
          ) : view === 'loupe' && current ? (
            <LoupeCanvas model={model} />
          ) : state.photos.length ? (
            <GridGallery model={model} />
          ) : (
            <EmptyCanvas model={model} />
          )}
        </div>

        <FilmstripDock model={model} />

        <WorkspaceStatus model={model} />
      </section>

      <InspectorPanel model={model} />
    </main>
  );
}
