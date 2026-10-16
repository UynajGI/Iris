import {
  EditingClient,
  type EditRecord,
  type EditSyncGroup,
  type SelectiveEditSync,
} from './editing-client.js';
import type { Photo } from './types.js';
import { ApiError } from './api-error.js';

export function syncErrorMessage(reason: unknown) {
  if (!(reason instanceof ApiError))
    return reason instanceof Error ? reason.message : String(reason);
  if (/revision conflict|changed while preparing/.test(reason.message))
    return '照片调整已发生变化，本次未同步。请刷新目标后重新选择。';
  if (/panel|binding|disconnected/.test(reason.message))
    return '所选版本的调色面板已禁用或不兼容，请启用面板或选择其他版本。';
  if (reason.status >= 500) return '暂时无法确认同步结果，请点击“重试同步”核对。';
  return '同步未完成，请刷新目标列表并检查所选照片是否可用。';
}

export const syncGroups: { id: EditSyncGroup; label: string }[] = [
  { id: 'tone', label: '影调' },
  { id: 'color', label: '颜色与 HSL' },
  { id: 'curves', label: '曲线' },
  { id: 'color_grading', label: '颜色分级' },
  { id: 'detail', label: '细节' },
  { id: 'effects', label: '颗粒与暗角' },
  { id: 'crop', label: '裁切与透视' },
  { id: 'optics', label: '光学校正' },
];
export const defaultSyncGroups: EditSyncGroup[] = [
  'tone',
  'color',
  'curves',
  'color_grading',
  'detail',
  'effects',
];
export type SyncCandidate = {
  photo: Photo;
  versions: EditRecord[];
  editId: string;
  selected: boolean;
};

/** Read four catalogs at a time; opening/cancelling the dialog never creates edits. */
export async function loadSyncCandidates(api: EditingClient, photos: Photo[], signal: AbortSignal) {
  const rows = new Array<SyncCandidate>(photos.length);
  let cursor = 0;
  const worker = async () => {
    while (cursor < photos.length && !signal.aborted) {
      const index = cursor++;
      const photo = photos[index]!;
      const versions = await api.list(photo.id, signal);
      rows[index] = {
        photo,
        versions,
        editId: versions.length === 1 ? versions[0]!.id : '',
        selected: false,
      };
    }
  };
  await Promise.all(Array.from({ length: Math.min(4, photos.length) }, worker));
  if (signal.aborted) throw new Error('读取已取消');
  return rows;
}

export function syncRequest(
  source: EditRecord,
  targets: EditRecord[],
  groups: EditSyncGroup[],
  requestId: string
): SelectiveEditSync {
  if (!targets.length || targets.length > 100 || !groups.length)
    throw new Error('请选择设置和 1–100 张目标照片');
  if (new Set(targets.map((target) => target.id)).size !== targets.length)
    throw new Error('目标版本重复');
  if (targets.some((target) => target.photo_id === source.photo_id))
    throw new Error('不能同步到来源照片');
  return {
    expected_revision: source.revision,
    request_id: requestId,
    targets: targets.map((target) => ({ id: target.id, expected_revision: target.revision })),
    groups: [...groups],
  };
}

/** Explicitly confirmed, previously untouched photos receive an initial version.
 * If any preparation fails, the UI must reload before another attempt. */
export async function prepareSyncTargets(
  api: EditingClient,
  candidates: SyncCandidate[]
): Promise<EditRecord[]> {
  const selected = candidates.filter((row) => row.selected);
  if (!selected.length || selected.length > 100) throw new Error('请选择 1–100 张目标照片');
  if (selected.some((row) => row.versions.length && !row.editId))
    throw new Error('请为每张照片选择目标版本');
  const targets: EditRecord[] = [];
  for (const row of selected) {
    if (row.versions.length) {
      const version = row.versions.find((item) => item.id === row.editId);
      if (!version) throw new Error('目标版本已变化，请刷新目标列表');
      targets.push(version);
    } else {
      // Detect versions created in another window since this dialog opened.
      if ((await api.list(row.photo.id)).length)
        throw new Error('目标照片已有新版本，请刷新后选择');
      targets.push(await api.create(row.photo.id));
    }
  }
  return targets;
}
