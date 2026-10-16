import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { IrisClient } from '../../client.js';
import { ApiError } from '../../api-error.js';
import { sourceDocument, cropPreviewDocument } from '../../editing-document.js';
import { preferredEdit, rememberEdit } from '../../editing-preferences.js';
import { EditingPreviewQueue } from '../../editing-preview-queue.js';
import {
  EditingClient,
  type EditDocument,
  type EditHistoryEntry,
  type EditRecord,
} from '../../editing-client.js';

const opening = new WeakMap<IrisClient, Map<number, Promise<EditRecord>>>();
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

export function useColorEditor(
  client: IrisClient,
  photoId: number | undefined,
  showBefore = false,
  cropMode = false,
  proofResource: string | null = null
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
  const [rendering, setRendering] = useState(false);
  const [preview, setPreview] = useState<string | null>(null);
  const [previewDocument, setPreviewDocument] = useState<EditDocument | null>(null);
  const mode = JSON.stringify([cropMode, proofResource]);
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
  const pendingSave = useRef<{
    record: EditRecord;
    document: EditDocument;
    requestId: string;
  } | null>(null);
  type PreviewInput = {
    record: EditRecord;
    document: EditDocument;
    sequence: number;
    edge: number;
    crop: boolean;
    proof: string | null;
  };
  const previewQueue = useRef<EditingPreviewQueue<PreviewInput, Blob> | null>(null);
  const activePreviewEdge = useRef(0);
  const previewKey = useRef('');
  const dirty = !!record && !!draft && JSON.stringify(record.document) !== JSON.stringify(draft);
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
    setPreviewError(null);
    setLoading(!!photoId);
    pendingSave.current = null;
    if (photoId === undefined) return;
    void openEdit(api, photoId)
      .then((value) => {
        if (current !== epoch.current) return;
        setRecord(value);
        setDraft(value.document);
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
    };
  }, [api, photoId, retry]);

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
    const queue = new EditingPreviewQueue<PreviewInput, Blob>({
      render: (input, signal) => {
        activePreviewEdge.current = input.edge;
        activeProof.current = input.proof;
        return api.preview(
          input.record,
          input.document,
          session,
          input.sequence,
          signal,
          input.edge,
          input.proof
        );
      },
      result: (blob, input) => {
        const current = latest.current.record;
        if (
          current?.id !== input.record.id ||
          current.revision !== input.record.revision ||
          current.photo_id !== requestedPhoto.current ||
          requestedMode.current !== JSON.stringify([input.crop, input.proof])
        )
          return;
        setPreviewError(null);
        setPreviewMode(JSON.stringify([input.crop, input.proof]));
        setPreviewDocument(input.document);
        const url = URL.createObjectURL(blob);
        setPreview((previous) => {
          if (previous) URL.revokeObjectURL(previous);
          return url;
        });
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
      if (previewQueue.current === queue) previewQueue.current = null;
    };
  }, [api, session]);
  useEffect(() => {
    if (!record || !draft || record.photo_id !== photoId) return;
    const document = cropMode ? cropPreviewDocument(draft) : draft;
    const key = JSON.stringify({
      document,
      id: record.id,
      revision: record.revision,
      session,
      previewAttempt,
      cropMode,
      proofResource,
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
      });
    }, 300);
    return () => clearTimeout(timer);
  }, [api, record, draft, session, photoId, previewAttempt, cropMode, proofResource]);
  const beforeKey = JSON.stringify({ source: draft?.source, output: draft?.output });
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
      .preview(record, sourceDocument(draft), `${session}-before`, number, controller.signal)
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
  useEffect(
    () => () => {
      if (preview) URL.revokeObjectURL(preview);
    },
    [preview]
  );

  const save = useCallback(async () => {
    if (mutation.current) return false;
    const state = latest.current;
    if (!state.record || !state.draft || state.record.photo_id !== requestedPhoto.current)
      return false;
    if (JSON.stringify(state.record.document) === JSON.stringify(state.draft)) return true;
    mutation.current = true;
    setBusy(true);
    setError(null);
    const current = epoch.current;
    const pending = pendingSave.current ?? {
      record: state.record,
      document: state.draft,
      requestId: crypto.randomUUID(),
    };
    pendingSave.current = pending;
    try {
      const saved = await api.save(pending.record, pending.document, pending.requestId);
      if (current !== epoch.current) return false;
      pendingSave.current = null;
      setRecord(saved);
      setDraft((value) => (value === pending.document ? saved.document : value));
      latest.current = {
        record: saved,
        draft: latest.current.draft === pending.document ? saved.document : latest.current.draft,
      };
      return JSON.stringify(latest.current.draft) === JSON.stringify(saved.document);
    } catch (reason) {
      if (isFinalRejection(reason)) pendingSave.current = null;
      if (current === epoch.current) setError(describe(reason));
      return false;
    } finally {
      mutation.current = false;
      if (current === epoch.current) setBusy(false);
    }
  }, [api]);

  const command = async (operation: 'undo' | 'redo' | 'duplicate') => {
    if (mutation.current || !latest.current.record) return;
    if (dirty && !(await save())) return;
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
    if (mutation.current || (dirty && !(await save()))) return;
    mutation.current = true;
    setBusy(true);
    setError(null);
    const current = epoch.current;
    try {
      const result = await api.get(id);
      if (current === epoch.current) {
        setRecord(result);
        setDraft(result.document);
      }
    } catch (reason) {
      if (current === epoch.current) setError(describe(reason));
    } finally {
      mutation.current = false;
      if (current === epoch.current) setBusy(false);
    }
  };
  const changeDraft = (document: EditDocument) => {
    if (latest.current.record?.photo_id !== requestedPhoto.current) return;
    latest.current = { ...latest.current, draft: document };
    if (typeof window !== 'undefined')
      window.document.documentElement.dataset.irisEditingDirty = String(
        JSON.stringify(latest.current.record?.document) !== JSON.stringify(document)
      );
    setDraft(document);
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
    rendering,
    preview: belongs && previewMode === mode ? preview : null,
    previewDocument,
    dirty: belongs && dirty,
    setDraft: changeDraft,
    save,
    command,
    selectVersion,
    beforePreview: belongs && beforeImage?.key === beforeKey ? beforeImage.url : null,
    beforeLoading,
    beforeError,
    retry: () => {
      if (record) setPreviewAttempt((value) => value + 1);
      else setRetry((value) => value + 1);
    },
    discard: () => {
      if (record) {
        pendingSave.current = null;
        setDraft(record.document);
      }
    },
  };
}
export type ColorEditor = ReturnType<typeof useColorEditor>;
