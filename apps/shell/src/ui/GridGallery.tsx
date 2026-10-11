import { type CSSProperties } from 'react';
import type { Photo } from '../types.js';
import { PhotoImage } from './components.js';
import { Icon } from './Icon.js';
import { colorNames } from './MarkTools.js';

import { ScoreChip, Stars, decisionIcons, decisionNames } from './photo-readouts.js';
import type { PhotoWorkspaceModel } from './usePhotoWorkspace.js';

export function GridGallery({ model }: { model: PhotoWorkspaceModel }) {
  const { tileInfo, tileSize, state } = model;
  return (<div className={`grid ${tileInfo ? '' : 'bare'}`} style={{ '--tile': `${tileSize}px` } as CSSProperties}>{state.photos.map((photo, index) => <PhotoTile key={photo.id} model={model} photo={photo} index={index} />)}</div>);
}

export function PhotoTile({ model, photo, index }: { model: PhotoWorkspaceModel; photo: Photo; index: number }) {
  const { state, clickPhoto, openLoupe, store, offset, toggleSelected, tileInfo } = model;
  const selected = model.state.selectedIds.includes(photo.id);
  return (<article key={photo.id} data-photo-id={photo.id} className={`cell ${photo.id === state.focusedId ? 'current' : ''} ${selected ? 'selected' : ''} decided-${photo.decision}`} style={{ '--i': Math.min(index, 24) } as CSSProperties}>
    <button className="cell-photo" onClick={event => clickPhoto(photo, index, event)} onDoubleClick={() => openLoupe(photo)} aria-label={`查看 ${photo.filename}，${decisionNames[photo.decision]}，${photo.rating ? `${photo.rating} 星` : '无星标'}，${colorNames[photo.color_label ?? 'none']}`}><PhotoImage client={store.client} photo={photo} /></button>
    <span className="cell-index tabular" aria-hidden="true">{offset + index + 1}</span>
    <label className="cell-check" title="选择"><input type="checkbox" aria-label={`选择 ${photo.filename}`} checked={selected} onChange={event => toggleSelected(photo.id, event.target.checked)} /></label>
    {photo.decision !== 'pending' && <span className={`cell-flag flag-${photo.decision}`} aria-hidden="true" title={decisionNames[photo.decision]}><Icon name={decisionIcons[photo.decision]} /></span>}
    <span className="cell-score-slot" aria-hidden="true"><ScoreChip photo={photo} /></span>
    {tileInfo && <div className="cell-meta" aria-hidden="true">
      <span className="filename-line">{photo.filename}</span>
      <span className="cell-meta-row">{photo.rating ? <Stars rating={photo.rating} /> : <span className="cell-dim">未评级</span>}<span className="cell-tags">{photo.color_label && photo.color_label !== 'none' && <span className={`color-dot color-${photo.color_label}`} />}<span className="cell-format">{photo.format.toUpperCase()}</span></span></span>
    </div>}
  </article>);
}
