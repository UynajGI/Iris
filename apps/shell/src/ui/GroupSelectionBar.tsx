import { useState } from 'react';
import { faceRegions } from '../selection-readout.js';
import type { Photo, GroupSelection } from '../types.js';
import type { GroupSelectionControl } from './useGroupSelection.js';

export function GroupSelectionBar({
  control,
  photos,
}: {
  control: GroupSelectionControl;
  photos: Photo[];
}) {
  const { selection, busy, error, adopt } = control;
  return (
    <div className="group-selection-bar" aria-busy={busy}>
      <TargetForm control={control} count={photos.length} />
      <MainFaceSelect control={control} photos={photos} />
      <button
        disabled={busy || !selection?.pending_keep_photo_ids.length}
        onClick={() => void adopt()}
      >
        保留建议的 {selection?.pending_keep_photo_ids.length ?? 0} 张
      </button>
      {selection && <SelectionSummary selection={selection} />}
      {error && (
        <p className="inline-error" role="alert">
          本组建议未更新：{error}
        </p>
      )}
    </div>
  );
}

function TargetForm({ control, count }: { control: GroupSelectionControl; count: number }) {
  const { selection, busy, save } = control;
  const [target, setTarget] = useState('');
  const intent = selection?.intent;
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        if (!intent) return;
        const value = Number(target || intent.target_keep || 1);
        if (Number.isSafeInteger(value) && value >= 1 && value <= count)
          void save({ ...intent, target_keep: value });
      }}
    >
      <label>
        本组保留{' '}
        <input
          aria-label="本组保留数量"
          type="number"
          min={1}
          max={count}
          value={target || intent?.target_keep || 1}
          disabled={busy || !selection}
          onChange={(event) => setTarget(event.target.value)}
        />
      </label>
      <button disabled={busy || !selection} type="submit">
        更新建议
      </button>
    </form>
  );
}

function MainFaceSelect({ control, photos }: { control: GroupSelectionControl; photos: Photo[] }) {
  const { selection, busy, save, chooseFace } = control;
  const intent = selection?.intent;
  const main = intent?.main_face;
  return (
    <label>
      主角人脸{' '}
      <select
        aria-label="本组主角人脸"
        disabled={busy || !selection}
        value={main ? `${main.photo_id}:${main.face_index}` : ''}
        onChange={(event) => {
          if (!intent) return;
          if (!event.target.value) void save({ ...intent, main_face: null, face_choices: [] });
          else {
            const [photo, index] = event.target.value.split(':').map(Number);
            void chooseFace(photo!, index!, true);
          }
        }}
      >
        <option value="">综合画面，不指定人物</option>
        {photos.flatMap((photo) =>
          faceRegions(photo).map((face, index) => (
            <option key={`${photo.id}:${face.index}`} value={`${photo.id}:${face.index}`}>
              {photo.filename} · 人脸 {index + 1}
            </option>
          ))
        )}
      </select>
    </label>
  );
}

function SelectionSummary({ selection }: { selection: GroupSelection }) {
  const intent = selection.intent;
  return (
    <p className="muted" role="status">
      已建议 {selection.recommended_photo_ids.length} 张 · 锁定{' '}
      {intent?.locked_photo_ids?.length ?? 0} 张
      {selection.remaining_slots > 0 && ` · 还缺 ${selection.remaining_slots} 张，需人工选取`}
      {selection.over_target > 0 && ` · 人工保留或锁定超出目标 ${selection.over_target} 张`}
      {intent.main_face && ' · 人脸位置对应仅为候选，请逐张确认'}
    </p>
  );
}
