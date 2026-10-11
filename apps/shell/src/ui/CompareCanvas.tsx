import { type CSSProperties } from 'react';
import { PhotoImage } from './components.js';
import { Icon } from './Icon.js';

import { DecisionBadge, ScoreChip, Stars } from './photo-readouts.js';
import type { PhotoWorkspaceModel } from './usePhotoWorkspace.js';
import { FullscreenButton } from './FullscreenButton.js';

export function CompareCanvas({ model }: { model: PhotoWorkspaceModel }) {
  const { fullscreen, stageRef, group, store } = model;
  if (!group) return null;
  return (<div className={`compare-stage ${fullscreen.active ? 'is-fullscreen' : ''}`} ref={stageRef} aria-busy={group.loading}>
    {group.loading && !group.photos.length ? <div className="canvas-empty" role="status"><span className="activity-ring large" aria-hidden="true" /><p>正在载入相似照片</p></div>
      : !group.comparisonIds.length ? <div className="canvas-empty"><Icon name="compare" className="state-icon" /><p>在下方胶片栏点选要对比的照片</p></div>
        : <div className="compare-grid" style={{ '--columns': Math.min(group.comparisonIds.length, 3) } as CSSProperties}>{group.photos.filter(photo => group.comparisonIds.includes(photo.id)).map(photo => <button key={photo.id} className="compare-cell" disabled={group.loading} aria-label={`查看 ${photo.filename}`} aria-pressed={photo.id === group.focusedId} onClick={() => store.focusGroupPhoto(photo.id)}>
          <span className="compare-frame"><PhotoImage client={store.client} photo={photo} kind="preview" /></span>
          <span className="compare-caption"><span className="filename-line">{photo.filename}</span><ScoreChip photo={photo} /><DecisionBadge decision={photo.decision} />{!!photo.rating && <Stars rating={photo.rating} />}</span>
        </button>)}</div>}
    <div className="stage-hud hud-corner">{group.loading && <span role="status">正在更新</span>}<FullscreenButton model={model} /></div>
  </div>);
}
