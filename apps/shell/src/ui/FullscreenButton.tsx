import { Icon } from './Icon.js';
import type { PhotoWorkspaceModel } from './usePhotoWorkspace.js';

export function FullscreenButton({ model }: { model: PhotoWorkspaceModel }) {
  const { fullscreen, setError } = model;
  if (!fullscreen.supported) return null;
  return (
    <button
      type="button"
      className="hud-button"
      aria-label={fullscreen.active ? '退出全屏' : '全屏'}
      title={fullscreen.active ? '退出全屏 (F / Esc)' : '全屏 (F)'}
      onClick={() => void fullscreen.toggle().catch((reason) => setError(String(reason)))}
    >
      <Icon name={fullscreen.active ? 'fullscreen-exit' : 'fullscreen'} />
    </button>
  );
}
