import type { PhotoWorkspaceModel } from './usePhotoWorkspace.js';
import { ZoomViewer } from './ZoomViewer.js';
import { ColorWorkspace, type EditorBridge } from './color/ColorWorkspace.js';

/** Workspace layout for the next phase; no image adjustments are applied yet. */
export function EditingWorkspace({
  model,
  stage,
  onDirty,
  registerSave,
}: {
  model: PhotoWorkspaceModel;
  stage: 'color' | 'retouch';
} & EditorBridge) {
  if (stage === 'color')
    return <ColorWorkspace model={model} onDirty={onDirty} registerSave={registerSave} />;
  const title = '修图';
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
