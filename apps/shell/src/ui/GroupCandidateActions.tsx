import type { Photo } from '../types.js';
import type { GroupSelectionControl } from './useGroupSelection.js';

export function GroupCandidateActions({
  control,
  photo,
}: {
  control: GroupSelectionControl;
  photo: Photo;
}) {
  const { selection, busy, save, chooseFace } = control;
  if (!selection) return null;
  const locks = selection.intent.locked_photo_ids ?? [];
  const locked = locks.includes(photo.id);
  const mayLock = locked || ['pending', 'keep'].includes(photo.decision);
  const mayMarkAbsent =
    selection.intent.main_face && selection.intent.main_face.photo_id !== photo.id;
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
      {mayMarkAbsent && (
        <button disabled={busy} onClick={() => void chooseFace(photo.id, null)}>
          此片主角未出现
        </button>
      )}
    </div>
  );
}
