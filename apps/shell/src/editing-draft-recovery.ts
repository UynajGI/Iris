// SPDX-License-Identifier: GPL-3.0-or-later
import type { EditingClient, EditDocument, EditRecord } from './editing-client.js';
import {
  EditingDrafts,
  resolveDraft,
  type EditDraft,
  type PendingEditSave,
} from './editing-drafts.js';

export type DraftRecovery = {
  id: string;
  message: string;
  conflict: boolean;
  cached: EditDraft | null;
};
export type LoadedDraft = {
  document: EditDocument;
  pending: PendingEditSave | null;
  recovery: DraftRecovery | null;
};

async function validateDraft(
  api: EditingClient,
  record: EditRecord,
  document: EditDocument,
  signal: AbortSignal
) {
  const session = `recover-${crypto.randomUUID()}`;
  try {
    // Validate the entire persisted graph with the authoritative renderer before
    // rendering its controls. This runs only on recovery, never while dragging.
    await api.preview(record, document, session, 1, signal, 384);
  } finally {
    void api.close(session).catch(() => {});
  }
}

export async function loadEditingDraft(
  api: EditingClient,
  store: EditingDrafts,
  record: EditRecord,
  signal: AbortSignal
): Promise<LoadedDraft> {
  const saved = { document: record.document, pending: null, recovery: null };
  let cached: EditDraft | null = null;
  try {
    cached = store.read(record.id);
    if (!cached) return saved;
    const resolution = resolveDraft(record, cached);
    if (resolution.kind === 'clean') {
      store.remove(record.id);
      return saved;
    }
    if (resolution.kind === 'conflict')
      return {
        ...saved,
        recovery: {
          id: record.id,
          cached,
          conflict: true,
          message: '此版本已有更新。未保存的调整可以恢复到独立副本。',
        },
      };
    await validateDraft(api, record, resolution.document, signal);
    if (signal.aborted) throw new DOMException('Aborted', 'AbortError');
    return { document: resolution.document, pending: resolution.pending, recovery: null };
  } catch (error) {
    if (signal.aborted) throw error;
    return {
      ...saved,
      recovery: {
        id: record.id,
        cached,
        conflict: false,
        message: `草稿尚未恢复：${error instanceof Error ? error.message : String(error)}`,
      },
    };
  }
}

export async function recoverDraftCopy(
  api: EditingClient,
  recovery: DraftRecovery,
  signal: AbortSignal
) {
  if (!recovery.cached) throw new Error('没有可以恢复的草稿');
  const current = await api.get(recovery.id);
  await validateDraft(api, current, recovery.cached.document, signal);
  if (signal.aborted) throw new DOMException('Aborted', 'AbortError');
  const record = await api.duplicate(current, '恢复的草稿');
  return { record, document: recovery.cached.document };
}
