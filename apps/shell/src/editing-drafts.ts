// SPDX-License-Identifier: GPL-3.0-or-later
import type { EditDocument, EditRecord } from './editing-client.js';

export type PendingEditSave = { record: EditRecord; document: EditDocument; requestId: string };
export type EditDraft = {
  format: 1;
  record: EditRecord;
  document: EditDocument;
  pending: PendingEditSave | null;
};
export type DraftStorage = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;
const prefix = 'iris.editing-draft.v1.';
const limit = 2 * 1024 * 1024;
export const sameDocument = (a: EditDocument, b: EditDocument) =>
  JSON.stringify(a) === JSON.stringify(b);

export function hasUnsavedDraft(
  record: EditRecord | null,
  draft: EditDocument | null,
  pending: PendingEditSave | null,
  recovering: boolean
) {
  return !!record && !!draft && (!!pending || recovering || !sameDocument(record.document, draft));
}

export function mergeSavedDraft(
  latest: EditDocument | null,
  submitted: EditDocument,
  saved: EditDocument
) {
  return latest && sameDocument(latest, submitted) ? saved : latest;
}

function object(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}
function document(value: unknown): value is EditDocument {
  return (
    object(value) &&
    object(value.source) &&
    typeof value.source.fingerprint === 'string' &&
    typeof value.source.photo_id === 'string' &&
    Array.isArray(value.nodes) &&
    Array.isArray(value.panel_bindings) &&
    object(value.output) &&
    object(value.node_positions) &&
    typeof value.output_node === 'string' &&
    Number.isInteger(value.schema_version)
  );
}
function record(value: unknown, id: string): value is EditRecord {
  return (
    object(value) &&
    value.id === id &&
    Number.isSafeInteger(value.photo_id) &&
    Number.isSafeInteger(value.revision) &&
    Number(value.revision) >= 0 &&
    document(value.document)
  );
}
function parse(raw: string, id: string): EditDraft {
  if (raw.length > limit) throw new Error('暂存草稿过大，无法恢复');
  const value: unknown = JSON.parse(raw);
  if (
    !object(value) ||
    value.format !== 1 ||
    !record(value.record, id) ||
    !document(value.document)
  )
    throw new Error('暂存草稿格式损坏');
  const pending = value.pending;
  if (
    pending !== null &&
    (!object(pending) ||
      !record(pending.record, id) ||
      !document(pending.document) ||
      typeof pending.requestId !== 'string' ||
      !/^[\da-f-]{36}$/i.test(pending.requestId))
  )
    throw new Error('暂存的保存请求格式损坏');
  return value as EditDraft;
}

/** Per-window recovery. Writes are coalesced; pagehide/unmount flush explicitly.
 * The backend validates recovered documents before any of their controls render.
 */
export class EditingDrafts {
  private entries = new Map<string, EditDraft | null>();
  private changed = new Set<string>();
  private timer: ReturnType<typeof setTimeout> | undefined;
  constructor(
    private storage: DraftStorage | null,
    private warning: (failed: boolean) => void = () => {}
  ) {}

  read(id: string): EditDraft | null {
    if (this.entries.has(id)) return this.entries.get(id)!;
    const raw = this.storage?.getItem(prefix + id);
    const value = raw ? parse(raw, id) : null;
    this.entries.set(id, value);
    return value;
  }
  write(record: EditRecord, document: EditDocument, pending: PendingEditSave | null = null) {
    // Keep pointer moves O(1); serialize and compare only in the coalesced flush.
    this.entries.set(record.id, { format: 1, record, document, pending });
    this.changed.add(record.id);
    if (!this.timer) this.timer = setTimeout(() => this.flush(), 250);
  }
  remove(id: string) {
    this.entries.set(id, null);
    this.changed.add(id);
    this.flush();
  }
  flush(): boolean {
    clearTimeout(this.timer);
    this.timer = undefined;
    if (!this.changed.size) return true;
    try {
      if (!this.storage) throw new Error('Session storage unavailable');
      for (const id of this.changed) {
        const value = this.entries.get(id);
        if (!value || (!value.pending && sameDocument(value.record.document, value.document))) {
          this.storage.removeItem(prefix + id);
          this.entries.set(id, null);
        } else {
          const raw = JSON.stringify(value);
          if (raw.length > limit) throw new Error('Draft exceeds storage budget');
          this.storage.setItem(prefix + id, raw);
        }
        this.changed.delete(id);
      }
      this.warning(false);
      return true;
    } catch {
      // Keep the in-memory draft and the last durable copy on quota/permission errors.
      this.warning(true);
      return false;
    }
  }
}

export type DraftResolution =
  | { kind: 'clean' }
  | { kind: 'restore'; document: EditDocument; pending: PendingEditSave | null }
  | { kind: 'conflict' };

export function resolveDraft(current: EditRecord, cached: EditDraft): DraftResolution {
  const base = cached.record;
  if (
    base.id !== current.id ||
    base.photo_id !== current.photo_id ||
    base.document.source.fingerprint !== current.document.source.fingerprint ||
    base.document.source.photo_id !== current.document.source.photo_id
  )
    return { kind: 'conflict' };
  if (!cached.pending && sameDocument(current.document, cached.document)) return { kind: 'clean' };
  if (current.revision === base.revision && sameDocument(current.document, base.document))
    return { kind: 'restore', document: cached.document, pending: cached.pending };
  const pending = cached.pending;
  if (
    pending &&
    current.revision === pending.record.revision + 1 &&
    sameDocument(current.document, pending.document)
  )
    return sameDocument(current.document, cached.document)
      ? { kind: 'clean' }
      : { kind: 'restore', document: cached.document, pending: null };
  return { kind: 'conflict' };
}
