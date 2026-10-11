import { useState, type CSSProperties } from 'react';
import { PhotoImage } from './components.js';
import { Icon } from './Icon.js';

import { DecisionBadge, ScoreChip, Stars } from './photo-readouts.js';
import type { PhotoWorkspaceModel } from './usePhotoWorkspace.js';
import { FullscreenButton } from './FullscreenButton.js';
import type { Photo } from '../types.js';
import { FaceDetail } from './FaceDetail.js';
import { mediaRevision } from '../media-cache.js';
import { selectionReason } from '../selection-readout.js';

export function CompareCanvas({ model }: { model: PhotoWorkspaceModel }) {
  const { fullscreen, stageRef, group, store } = model;
  const [details, setDetails] = useState(false);
  if (!group) return null;
  const assessment = model.state.groups.find((item) => item.id === group.groupId)?.assessment;
  return (
    <div
      className={`compare-stage ${fullscreen.active ? 'is-fullscreen' : ''}`}
      ref={stageRef}
      aria-busy={group.loading}
    >
      <div className="compare-options">
        <div className="segmented" role="group" aria-label="比较范围">
          <button aria-pressed={!details} onClick={() => setDetails(false)}>
            整图
          </button>
          <button aria-pressed={details} onClick={() => setDetails(true)}>
            人脸细节
          </button>
        </div>
        <p className="muted">
          {details
            ? '各图独立选择人脸区域，请确认是同一人物'
            : assessment && assessment.candidate_photo_ids.length > 1
              ? '多张技术质量接近，可保留不同表情与动作'
              : '组内技术参考 · 最终取舍由你决定'}
        </p>
      </div>
      {group.loading && !group.photos.length ? (
        <div className="canvas-empty" role="status">
          <span className="activity-ring large" aria-hidden="true" />
          <p>正在载入相似照片</p>
        </div>
      ) : !group.comparisonIds.length ? (
        <div className="canvas-empty">
          <Icon name="compare" className="state-icon" />
          <p>在下方胶片栏点选要对比的照片</p>
        </div>
      ) : (
        <div
          className="compare-grid"
          style={{ '--columns': Math.min(group.comparisonIds.length, 3) } as CSSProperties}
        >
          {group.photos
            .filter((photo) => group.comparisonIds.includes(photo.id))
            .map((photo) => (
              <ComparePhoto
                key={`${photo.id}:${mediaRevision(photo)}`}
                photo={photo}
                model={model}
                details={details}
                reasons={
                  assessment?.comparisons.find((item) => item.photo_id === photo.id)?.reasons ?? []
                }
              />
            ))}
        </div>
      )}
      <div className="stage-hud hud-corner">
        {group.loading && <span role="status">正在更新</span>}
        <FullscreenButton model={model} />
      </div>
    </div>
  );
}

function ComparePhoto({
  photo,
  model,
  details,
  reasons,
}: {
  photo: Photo;
  model: PhotoWorkspaceModel;
  details: boolean;
  reasons: string[];
}) {
  const group = model.group!;
  const focus = () => model.store.focusGroupPhoto(photo.id);
  return (
    <section className="compare-cell" data-focused={photo.id === group.focusedId}>
      {details ? (
        <FaceDetail
          client={model.store.client}
          photo={photo}
          active={photo.id === group.focusedId && !group.loading}
        />
      ) : (
        <button
          className="compare-frame compare-image-button"
          disabled={group.loading}
          aria-label={`查看 ${photo.filename}`}
          aria-pressed={photo.id === group.focusedId}
          onClick={focus}
        >
          <PhotoImage client={model.store.client} photo={photo} kind="preview" />
        </button>
      )}
      <button
        className="compare-caption"
        disabled={group.loading}
        onClick={focus}
        aria-label={`选中 ${photo.filename}`}
        aria-pressed={photo.id === group.focusedId}
      >
        <span className="filename-line">{photo.filename}</span>
        <ScoreChip photo={photo} />
        <DecisionBadge decision={photo.decision} />
        {!!photo.rating && <Stars rating={photo.rating} />}
      </button>
      {reasons.length > 0 && (
        <ul className="compare-reasons">
          {reasons.map((code) => (
            <li key={code}>{selectionReason(code)}</li>
          ))}
        </ul>
      )}
    </section>
  );
}
