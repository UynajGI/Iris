import { useEffect, useMemo, useRef, useState } from 'react';
import type { IrisClient } from '../../client.js';
import { ApiError } from '../../api-error.js';
import {
  EditingClient,
  type EditRecord,
  type EditSyncGroup,
  type SelectiveEditSync,
} from '../../editing-client.js';
import {
  defaultSyncGroups,
  loadSyncCandidates,
  prepareSyncTargets,
  syncRequest,
  syncErrorMessage,
  type SyncCandidate,
} from '../../editing-sync.js';
import type { Photo } from '../../types.js';

export type SyncProps = {
  client: IrisClient;
  sourceId: string;
  photos: Photo[];
  prepareSource(): Promise<boolean>;
  onClose(): void;
};
function validSelection(rows: SyncCandidate[], groups: EditSyncGroup[]) {
  const selected = rows.filter((row) => row.selected);
  return (
    groups.length > 0 &&
    selected.length > 0 &&
    selected.length <= 100 &&
    selected.every((row) => !row.versions.length || row.editId)
  );
}

export function useSyncAdjustments({ client, sourceId, photos, prepareSource }: SyncProps) {
  const api = useMemo(() => new EditingClient(client), [client]);
  const [source, setSource] = useState<EditRecord | null>(null);
  const [rows, setRows] = useState<SyncCandidate[]>([]);
  const [groups, setGroups] = useState<EditSyncGroup[]>([...defaultSyncGroups]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [refresh, setRefresh] = useState(0);
  const [needsRefresh, setNeedsRefresh] = useState(false);
  const [completed, setCompleted] = useState<number | null>(null);
  const pending = useRef<SelectiveEditSync | null>(null);
  const running = useRef(false);
  useEffect(() => {
    let active = true;
    const controller = new AbortController();
    setLoading(true);
    setError(null);
    void (async () => {
      if (!(await prepareSource())) throw new Error('请先保存当前照片的调整');
      const current = await api.get(sourceId);
      const candidates = await loadSyncCandidates(
        api,
        photos.filter((photo) => photo.id !== current.photo_id),
        controller.signal
      );
      if (active) {
        setSource(current);
        setRows(candidates);
        setNeedsRefresh(false);
      }
    })()
      .catch((reason) => {
        if (active) {
          setError(syncErrorMessage(reason));
          setNeedsRefresh(true);
        }
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
      controller.abort();
    };
  }, [api, sourceId, refresh]);
  const submit = async () => {
    if (running.current || !source) return;
    running.current = true;
    setBusy(true);
    setError(null);
    try {
      if (!pending.current) {
        const targets = await prepareSyncTargets(api, rows);
        pending.current = syncRequest(source, targets, groups, crypto.randomUUID());
      }
      const result = await api.sync(source.id, pending.current);
      setCompleted(result.length);
      pending.current = null;
    } catch (reason) {
      const rejected = reason instanceof ApiError && reason.status >= 400 && reason.status < 500;
      if (rejected) pending.current = null;
      setNeedsRefresh(!pending.current);
      setError(syncErrorMessage(reason));
    } finally {
      running.current = false;
      setBusy(false);
    }
  };
  const locked = loading || busy || !!pending.current || completed !== null;
  return {
    source,
    rows,
    setRows,
    groups,
    setGroups,
    loading,
    busy,
    error,
    needsRefresh,
    completed,
    locked,
    retrying: !!pending.current,
    canSubmit: !locked && !needsRefresh && validSelection(rows, groups),
    refresh: () => setRefresh((value) => value + 1),
    submit,
    selectedCount: rows.filter((row) => row.selected).length,
  };
}
export type SyncModel = ReturnType<typeof useSyncAdjustments>;
