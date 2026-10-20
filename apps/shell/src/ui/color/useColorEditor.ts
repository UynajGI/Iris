import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { IrisClient } from '../../client.js';
import { ApiError } from '../../api-error.js';
import { sourceDocument, cropPreviewDocument } from '../../editing-document.js';
import { preferredEdit, rememberEdit } from '../../editing-preferences.js';
import { EditingPreviewQueue } from '../../editing-preview-queue.js';
import { EditingSaveQueue } from '../../editing-save-queue.js';
import { PreviewFrames } from '../../preview-frames.js';
import { observationDocument, type ObservationMode } from '../../editing-observation.js';
import { bypassAdjustmentGroup, type AdjustmentGroup } from '../../editing-groups.js';
import {
  EditingDrafts,
  sameDocument,
  hasUnsavedDraft,
  mergeSavedDraft,
  type PendingEditSave,
} from '../../editing-drafts.js';
import {
  loadEditingDraft,
  recoverDraftCopy,
  type DraftRecovery,
} from '../../editing-draft-recovery.js';
import {
  EditingClient,
  type EditDocument,
  type EditHistoryEntry,
  type EditRecord,
} from '../../editing-client.js';

const opening = new WeakMap<IrisClient, Map<number, Promise<EditRecord>>>();
type PreviewInput = {
  record: EditRecord;
  document: EditDocument;
  sequence: number;
  edge: number;
  crop: boolean;
  proof: string | null;
  observation: ObservationMode;
  bypass: AdjustmentGroup | null;
};
function modeKey(input: Pick<PreviewInput, 'crop' | 'proof' | 'observation' | 'bypass'>) {
  return JSON.stringify([input.crop, input.proof, input.observation, input.bypass]);
}
function openEdit(client: EditingClient, photo: number): Promise<EditRecord> {
  let map = opening.get(client.client);
  if (!map) {
    map = new Map();
    opening.set(client.client, map);
  }
  const pending = map.get(photo);
  if (pending) return pending;
  const promise = client
    .list(photo)
    .then((records) => preferredEdit(photo, records) ?? client.create(photo));
  map.set(photo, promise);
  void promise.finally(() => map!.delete(photo)).catch(() => {});
  return promise;
}
function describe(error: unknown) {
  if (error instanceof ApiError && error.status === 404) return '调色服务尚未就绪';
  return error instanceof Error ? error.message : String(error);
}
function isFinalRejection(error: unknown): boolean {
  return error instanceof ApiError && error.status >= 400 && error.status < 500;
}

function usePointerInteraction() {
  const [interacting, setInteracting] = useState(false);
  useEffect(() => {
    const start = () => setInteracting(true);
    const finish = () => setInteracting(false);
    window.addEventListener('pointerdown', start, true);
    window.addEventListener('pointerup', finish, true);
    window.addEventListener('pointercancel', finish, true);
    window.addEventListener('blur', finish);
    return () => {
      window.removeEventListener('pointerdown', start, true);
      window.removeEventListener('pointerup', finish, true);
      window.removeEventListener('pointercancel', finish, true);
      window.removeEventListener('blur', finish);
    };
  }, []);
  return interacting;
}

export function useColorEditor(
  client: IrisClient,
  photoId: number | undefined,
  showBefore = false,
  cropMode = false,
  proofResource: string | null = null,
  observation: ObservationMode = 'full',
  bypass: AdjustmentGroup | null = null
) {
  const api = useMemo(() => new EditingClient(client), [client]);
  const [record, setRecord] = useState<EditRecord | null>(null);
  const [versions, setVersions] = useState<EditRecord[]>([]);
  const [draft, setDraft] = useState<EditDocument | null>(null);
  const [history, setHistory] = useState<EditHistoryEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [saving, setSaving] = useState(false);
  const interacting = usePointerInteraction();
  const saveQueue = useMemo(() => new EditingSaveQueue(), []);
  const [recovery, setRecovery] = useState<DraftRecovery | null>(null);
  const recoveryRef = useRef(recovery);
  recoveryRef.current = recovery;
  const [draftStorageFailed, setDraftStorageFailed] = useState(false);
  const drafts = useMemo(() => {
    let storage: Storage | null = null;
    try {
      storage = window.sessionStorage;
    } catch {
      /* In-memory recovery remains available. */
    }
    return new EditingDrafts(storage, setDraftStorageFailed);
  }, []);
  useEffect(() => {
    const flush = () => {
      drafts.flush();
    };
    const visibility = () => {
      if (document.visibilityState === 'hidden') flush();
    };
    window.addEventListener('pagehide', flush);
    document.addEventListener('visibilitychange', visibility);
    return () => {
      window.removeEventListener('pagehide', flush);
      document.removeEventListener('visibilitychange', visibility);
      flush();
    };
  }, [drafts]);
  const [rendering, setRendering] = useState(false);
  const [preview, setPreview] = useState<string | null>(null);
  const [previewDocument, setPreviewDocument] = useState<EditDocument | null>(null);
  const mode = JSON.stringify([cropMode, proofResource, observation, bypass]);
  const [previewMode, setPreviewMode] = useState('');
  const requestedMode = useRef(mode);
  requestedMode.current = mode;
  const activeProof = useRef<string | null>(null);
  const [retry, setRetry] = useState(0);
  const [previewAttempt, setPreviewAttempt] = useState(0);
  const [beforeImage, setBeforeImage] = useState<{ key: string; url: string } | null>(null);
  const [beforeLoading, setBeforeLoading] = useState(false);
  const [beforeError, setBeforeError] = useState<string | null>(null);
  const epoch = useRef(0);
  const requestedPhoto = useRef(photoId);
  requestedPhoto.current = photoId;
  const sequence = useRef(0);
  const session = useMemo(() => crypto.randomUUID(), [photoId]);
  const latest = useRef({ record, draft });
  latest.current = { record, draft };
  const mutation = useRef(false);
  const pendingSave = useRef<PendingEditSave | null>(null);
  const recoveryRequest = useRef<AbortController | null>(null);
  const previewQueue = useRef<EditingPreviewQueue<PreviewInput, Blob> | null>(null);
  const activePreviewEdge = useRef(0);
  const previewKey = useRef('');
  const dirty = hasUnsavedDraft(record, draft, pendingSave.current, !!recovery);
  useEffect(() => {
    if (record) rememberEdit(record);
  }, [record?.id]);

  useEffect(() => {
    const current = ++epoch.current;
    setRecord(null);
    setDraft(null);
    setPreview(null);
    setBeforeImage(null);
    setHistory([]);
    setVersions([]);
    setBusy(false);
    setRendering(false);
    setError(null);
    setRecovery(null);
    setPreviewError(null);
    setLoading(!!photoId);
    pendingSave.current = null;
    if (photoId === undefined) return;
    const controller = new AbortController();
    void openEdit(api, photoId)
      .then(async (value) => {
        if (current !== epoch.current) return;
        const loaded = await loadEditingDraft(api, drafts, value, controller.signal);
        if (current !== epoch.current) return;
        pendingSave.current = loaded.pending;
        setRecovery(loaded.recovery);
        setRecord(value);
        setDraft(loaded.document);
        if (!loaded.recovery) drafts.write(value, loaded.document, loaded.pending);
        void api
          .list(photoId)
          .then((items) => {
            if (current === epoch.current) setVersions(items);
          })
          .catch(() => {});
      })
      .catch((reason) => {
        if (current === epoch.current) setError(describe(reason));
      })
      .finally(() => {
        if (current === epoch.current) setLoading(false);
      });
    return () => {
      ++epoch.current;
      controller.abort();
      recoveryRequest.current?.abort();
    };
  }, [api, photoId, retry, drafts]);

  useEffect(() => {
    if (!record) return;
    const controller = new AbortController();
    let active = true;
    void api
      .history(record.id, controller.signal)
      .then((items) => {
        if (active) setHistory(items);
      })
      .catch(() => {});
    return () => {
      active = false;
      controller.abort();
    };
  }, [api, record?.id, record?.revision]);

  useEffect(() => {
    previewKey.current = '';
    const frames = new PreviewFrames();
    const queue = new EditingPreviewQueue<PreviewInput, Blob>({
      render: (input, signal) => {
        activePreviewEdge.current = input.edge;
        activeProof.current = input.proof;
        return api.preview(
          input.record,
          observationDocument(input.document, input.observation),
          session,
          input.sequence,
          signal,
          input.edge,
          input.proof
        );
      },
      result: (blob, input) => {
        void frames.show(
          blob,
          (url) => {
            const current = latest.current.record;
            if (
              current?.id !== input.record.id ||
              current.revision !== input.record.revision ||
              current.photo_id !== requestedPhoto.current ||
              requestedMode.current !== modeKey(input)
            )
              return false;
            setPreviewError(null);
            setPreviewMode(modeKey(input));
            setPreviewDocument(input.document);
            setPreview(url);
            return true;
          },
          (reason) => setPreviewError(describe(reason))
        );
      },
      error: (reason) => setPreviewError(describe(reason)),
      busy: setRendering,
      cancel: (input) => {
        void api.cancel(session, input.sequence).catch(() => {});
      },
    });
    previewQueue.current = queue;
    return () => {
      queue.dispose();
      frames.dispose();
      if (previewQueue.current === queue) previewQueue.current = null;
    };
  }, [api, session]);
  useEffect(() => {
    if (!record || !draft || record.photo_id !== photoId) return;
    const projected = bypassAdjustmentGroup(draft, bypass);
    const document = cropMode ? cropPreviewDocument(projected) : projected;
    const key = JSON.stringify({
      document,
      id: record.id,
      revision: record.revision,
      session,
      previewAttempt,
      cropMode,
      proofResource,
      observation,
      bypass,
    });
    if (previewKey.current === key) return;
    previewKey.current = key;
    const queue = previewQueue.current;
    queue?.push(
      {
        record,
        document,
        sequence: ++sequence.current,
        edge: 384,
        crop: cropMode,
        proof: proofResource,
        observation,
        bypass,
      },
      activePreviewEdge.current > 384 || activeProof.current !== proofResource
    );
    const timer = setTimeout(() => {
      queue?.push({
        record,
        document,
        sequence: ++sequence.current,
        edge: 1600,
        crop: cropMode,
        proof: proofResource,
        observation,
        bypass,
      });
    }, 300);
    return () => clearTimeout(timer);
  }, [api, record, draft, session, photoId, previewAttempt, mode]);
  const beforeKey = JSON.stringify({ source: draft?.source, output: draft?.output, observation });
  useEffect(() => {
    if (!showBefore || !record || !draft || beforeImage?.key === beforeKey) {
      setBeforeLoading(false);
      return;
    }
    const controller = new AbortController();
    let active = true;
    const number = ++sequence.current;
    setBeforeLoading(true);
    setBeforeError(null);
    void api
      .preview(
        record,
        observationDocument(sourceDocument(draft), observation),
        `${session}-before`,
        number,
        controller.signal
      )
      .then((blob) => {
        if (active) setBeforeImage({ key: beforeKey, url: URL.createObjectURL(blob) });
      })
      .catch((reason) => {
        if (active) setBeforeError(describe(reason));
      })
      .finally(() => {
        if (active) setBeforeLoading(false);
      });
    return () => {
      active = false;
      controller.abort();
      void api.cancel(`${session}-before`, number).catch(() => {});
    };
  }, [api, showBefore, record, beforeKey, session, previewAttempt]);
  useEffect(
    () => () => {
      if (beforeImage) URL.revokeObjectURL(beforeImage.url);
    },
    [beforeImage]
  );
  useEffect(
    () => () => {
      void api.close(session).catch(() => {});
      void api.close(`${session}-before`).catch(() => {});
    },
    [api, session]
  );

  const rejectSave = useCallback(
    (reason: unknown, startedAt: number, base: EditRecord) => {
      if (startedAt !== epoch.current) return;
      if (isFinalRejection(reason)) {
        pendingSave.current = null;
        if (latest.current.draft) drafts.write(base, latest.current.draft);
      }
      setError(describe(reason));
    },
    [drafts]
  );
  const acceptSave = useCallback(
    (saved: EditRecord, submitted: EditDocument) => {
      const remaining = mergeSavedDraft(latest.current.draft, submitted, saved.document);
      pendingSave.current = null;
      latest.current = { record: saved, draft: remaining };
      setRecord(saved);
      setDraft(remaining);
      if (remaining) drafts.write(saved, remaining);
      drafts.flush();
      return !!remaining && sameDocument(remaining, saved.document);
    },
    [drafts]
  );
  const save = useCallback(
    () =>
      saveQueue.flush(async () => {
        if (mutation.current || recoveryRef.current) return 'failed';
        const state = latest.current;
        if (!state.record || !state.draft || state.record.photo_id !== requestedPhoto.current)
          return 'failed';
        if (!pendingSave.current && sameDocument(state.record.document, state.draft))
          return 'clean';
        mutation.current = true;
        setSaving(true);
        setError(null);
        const current = epoch.current;
        const pending = pendingSave.current ?? {
          record: state.record,
          document: state.draft,
          requestId: crypto.randomUUID(),
        };
        pendingSave.current = pending;
        drafts.write(state.record, state.draft, pending);
        drafts.flush();
        try {
          const saved = await api.save(pending.record, pending.document, pending.requestId);
          if (current !== epoch.current) return 'failed';
          return acceptSave(saved, pending.document) ? 'clean' : 'pending';
        } catch (reason) {
          rejectSave(reason, current, state.record);
          return 'failed';
        } finally {
          mutation.current = false;
          setSaving(false);
        }
      }),
    [api, drafts, acceptSave, rejectSave, saveQueue]
  );

  useEffect(() => {
    if (!dirty || loading || busy || saving || interacting || recovery || error) return;
    const timer = setTimeout(() => {
      void save();
    }, 650);
    return () => clearTimeout(timer);
  }, [draft, record, dirty, loading, busy, saving, interacting, recovery, error, save]);

  const command = async (operation: 'undo' | 'redo' | 'duplicate') => {
    if (recoveryRef.current || !latest.current.record || !(await save()) || mutation.current)
      return;
    const value = latest.current.record;
    if (!value) return;
    mutation.current = true;
    setBusy(true);
    setError(null);
    const current = epoch.current;
    try {
      const result =
        operation === 'duplicate'
          ? await api.duplicate(value, `虚拟副本 ${versions.length + 1}`)
          : await api.command(value, operation, crypto.randomUUID());
      if (current === epoch.current) {
        setRecord(result);
        setDraft(result.document);
        if (operation === 'duplicate') setVersions((items) => [...items, result]);
      }
    } catch (reason) {
      if (current === epoch.current) setError(describe(reason));
    } finally {
      mutation.current = false;
      if (current === epoch.current) setBusy(false);
    }
  };
  const selectVersion = async (id: string) => {
    if (recoveryRef.current || !(await save()) || mutation.current) return;
    mutation.current = true;
    setBusy(true);
    setError(null);
    const current = epoch.current;
    try {
      const result = await api.get(id);
      const loaded = await loadEditingDraft(api, drafts, result, new AbortController().signal);
      if (current === epoch.current) {
        pendingSave.current = loaded.pending;
        setRecovery(loaded.recovery);
        setRecord(result);
        setDraft(loaded.document);
        if (!loaded.recovery) drafts.write(result, loaded.document, loaded.pending);
      }
    } catch (reason) {
      if (current === epoch.current) setError(describe(reason));
    } finally {
      mutation.current = false;
      if (current === epoch.current) setBusy(false);
    }
  };
  const changeDraft = (document: EditDocument) => {
    if (recoveryRef.current || latest.current.record?.photo_id !== requestedPhoto.current) return;
    latest.current = { ...latest.current, draft: document };
    if (latest.current.record) drafts.write(latest.current.record, document, pendingSave.current);
    if (typeof window !== 'undefined')
      window.document.documentElement.dataset.irisEditingDirty = String(
        !!pendingSave.current ||
          JSON.stringify(latest.current.record?.document) !== JSON.stringify(document)
      );
    setDraft(document);
    setError(null);
  };
  const recoverCopy = async () => {
    const savedRecovery = recoveryRef.current;
    if (mutation.current || !savedRecovery) return;
    mutation.current = true;
    setBusy(true);
    setError(null);
    const current = epoch.current;
    const controller = new AbortController();
    recoveryRequest.current = controller;
    try {
      const restored = await recoverDraftCopy(api, savedRecovery, controller.signal);
      if (current !== epoch.current) return;
      drafts.write(restored.record, restored.document);
      if (drafts.flush()) drafts.remove(savedRecovery.id);
      pendingSave.current = null;
      latest.current = { record: restored.record, draft: restored.document };
      setRecord(restored.record);
      setDraft(restored.document);
      setVersions((items) => [...items, restored.record]);
      setRecovery(null);
    } catch (reason) {
      if (current === epoch.current) setError(describe(reason));
    } finally {
      if (recoveryRequest.current === controller) recoveryRequest.current = null;
      mutation.current = false;
      if (current === epoch.current) setBusy(false);
    }
  };
  const belongs = record?.photo_id === photoId;
  return {
    record: belongs ? record : null,
    versions: belongs ? versions : [],
    draft: belongs ? draft : null,
    history: belongs ? history : [],
    error,
    previewError,
    loading: loading || (photoId !== undefined && !belongs && !error),
    busy,
    recovery,
    draftStorageFailed,
    recoverCopy,
    rendering,
    preview: belongs ? preview : null,
    previewReady: belongs && previewMode === mode,
    previewDocument,
    dirty: belongs && dirty,
    setDraft: changeDraft,
    save,
    command,
    selectVersion,
    // Keep the decoded comparison frame while a new observation view is rendering.
    beforePreview: belongs ? (beforeImage?.url ?? null) : null,
    beforeLoading,
    beforeError,
    retry: () => {
      if (recovery) setRetry((value) => value + 1);
      else if (record) setPreviewAttempt((value) => value + 1);
      else setRetry((value) => value + 1);
    },
    discard: () => {
      if (record) {
        drafts.remove(record.id);
        pendingSave.current = null;
        setRecovery(null);
        setError(null);
        latest.current = { record, draft: record.document };
        setDraft(record.document);
      }
    },
  };
}
export type ColorEditor = ReturnType<typeof useColorEditor>;
