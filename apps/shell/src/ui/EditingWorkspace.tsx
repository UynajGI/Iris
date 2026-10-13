import type { PhotoWorkspaceModel } from './usePhotoWorkspace.js';
import { ZoomViewer } from './ZoomViewer.js';

/** Workspace layout for the next phase; no image adjustments are applied yet. */
export function EditingWorkspace({
  model,
  stage,
}: {
  model: PhotoWorkspaceModel;
  stage: 'color' | 'retouch';
}) {
  const title = stage === 'color' ? '调色' : '修图';
  return (
    <main className="editing-workspace" aria-label={`${title}工作区`}>
      <section className="editing-photo" aria-label="照片预览">
        {model.current ? (
          <ZoomViewer key={model.current.id} client={model.store.client} photo={model.current} />
        ) : (
          <p className="editing-empty">暂无照片</p>
        )}
      </section>
      <aside className="editing-tools">
        <h1>{title}</h1>
        <p>工具待接入</p>
      </aside>
    </main>
  );
}
