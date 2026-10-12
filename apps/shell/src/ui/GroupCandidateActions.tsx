import type { Photo } from '../types.js';
import type { GroupSelectionControl } from './useGroupSelection.js';

export function GroupCandidateActions({
  control,
  photo,
}: {
  control: GroupSelectionControl;
  photo: Photo;
}) {
  const { selection, busy, save } = control;
  if (!selection) return null;
  const locks = selection.intent.locked_photo_ids ?? [];
  const locked = locks.includes(photo.id);
  const mayLock = locked || ['pending', 'keep'].includes(photo.decision);
  const toggleLock = () =>
    save({
      ...selection.intent,
      locked_photo_ids: locked ? locks.filter((id) => id !== photo.id) : [...locks, photo.id],
    });
  return (
    <div className="group-candidate-actions">
      <button aria-pressed={locked} disabled={busy || !mayLock} onClick={() => void toggleLock()}>
        {locked ? '解除必留锁定' : '锁定必留'}
      </button>
      {selection.recommended_photo_ids.includes(photo.id) && <span>建议保留</span>}
      <MainFaceActions control={control} photoId={photo.id} />
    </div>
  );
}

function MainFaceActions({
  control,
  photoId,
}: {
  control: GroupSelectionControl;
  photoId: number;
}) {
  const { selection, busy, chooseFace, save } = control;
  if (!selection?.intent.main_face || selection.intent.main_face.photo_id === photoId) return null;
  const face = selection.faces.find((face) => face.photo_id === photoId);
  const index = face?.face_index ?? null;
  const corrections = selection.intent.face_choices ?? [];
  const corrected = corrections.some((choice) => choice.photo_id === photoId);
  return (
    <>
      <button disabled={busy || index === null} onClick={() => void chooseFace(photoId, index)}>
        {face?.state === 'manual' ? '重新确认主角' : '确认这是主角'}
      </button>
      <button disabled={busy} onClick={() => void chooseFace(photoId, null)}>
        此片主角未出现
      </button>
      {corrected && (
        <button
          disabled={busy}
          onClick={() =>
            void save({
              ...selection.intent,
              face_choices: corrections.filter((choice) => choice.photo_id !== photoId),
            })
          }
        >
          恢复位置候选
        </button>
      )}
    </>
  );
}
