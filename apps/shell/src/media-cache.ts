import type { IrisClient } from './client.js';
import type { Photo } from './types.js';

type Kind = 'thumb' | 'preview' | 'original';
type Entry = {
  controller: AbortController;
  promise: Promise<Blob>;
  users: number;
  bytes: number;
  settled: boolean;
  kind: Kind;
  refreshedAt: number;
  priority: number;
  started: boolean;
  start(): void;
  reject(error: Error): void;
};
export interface MediaLease {
  promise: Promise<Blob>;
  release(): void;
  invalidate(): void;
}

/** Cache compressed media, not decoded full-resolution bitmaps. Active leases own requests. */
export class MediaCache {
  private entries = new Map<string, Entry>();
  private running = 0;
  private background = 0;
  constructor(
    private source: Pick<IrisClient, 'media'>,
    private maxEntries = 24,
    private maxBytes = 32 * 1024 * 1024,
    private maxAgeMs = 30_000,
    private now = Date.now
  ) {}
  acquire(id: number, kind: Kind, revision = '', prefetch = false): MediaLease {
    const key = JSON.stringify([id, kind, revision]);
    let entry = this.entries.get(key);
    if (entry && entry.users === 0 && this.now() - entry.refreshedAt >= this.maxAgeMs) {
      this.remove(key, entry);
      entry = undefined;
    }
    if (!entry) {
      entry = this.create(id, kind, prefetch);
      this.entries.set(key, entry);
      const captured = entry;
      void entry.promise.then(
        (blob) => {
          captured.settled = true;
          captured.bytes = blob.size;
          captured.refreshedAt = this.now();
          if (captured.users === 0 && kind === 'original') this.remove(key, captured);
          this.trim();
        },
        () => this.remove(key, captured)
      );
    }
    this.entries.delete(key);
    this.entries.set(key, entry);
    entry.users++;
    // Promote a queued neighbor when it becomes the focused image.
    if (!prefetch) entry.priority = Math.min(entry.priority, kind === 'thumb' ? 1 : 0);
    this.dispatch();
    let released = false;
    return {
      promise: entry.promise,
      invalidate: () => this.remove(key, entry),
      release: () => {
        if (released) return;
        released = true;
        entry.users--;
        if (entry.users === 0 && (!entry.settled || kind === 'original')) this.remove(key, entry);
        this.trim();
      },
    };
  }
  private create(id: number, kind: Kind, prefetch: boolean): Entry {
    const controller = new AbortController();
    let resolve!: (blob: Blob) => void;
    let reject!: (error: Error) => void;
    const promise = new Promise<Blob>((yes, no) => {
      resolve = yes;
      reject = no;
    });
    const entry: Entry = {
      controller,
      promise,
      users: 0,
      bytes: 0,
      settled: false,
      kind,
      refreshedAt: this.now(),
      priority: prefetch ? 2 : kind === 'thumb' ? 1 : 0,
      started: false,
      reject,
      start: () => {
        entry.started = true;
        const background = entry.priority === 2;
        this.running++;
        if (background) this.background++;
        // Call immediately for shared leases; also handle synchronous transport errors.
        let request: Promise<Blob>;
        try {
          request = this.source.media(id, kind, controller.signal, background);
        } catch (error) {
          request = Promise.reject(error);
        }
        void request.then(resolve, reject).finally(() => {
          this.running--;
          if (background) this.background--;
          this.dispatch();
        });
      },
    };
    return entry;
  }
  private dispatch() {
    while (this.running < 4) {
      const next = [...this.entries.values()]
        .filter(
          (entry) =>
            !entry.started && entry.users > 0 && (entry.priority < 2 || this.background === 0)
        )
        .sort((a, b) => a.priority - b.priority)[0];
      if (!next) break;
      // Reserve one HTTP slot for a newly focused image.
      if (this.running >= 3 && next.priority > 0) break;
      next.start();
    }
  }
  private remove(key: string, entry: Entry) {
    if (this.entries.get(key) !== entry) return;
    entry.controller.abort();
    this.entries.delete(key);
    if (!entry.started) entry.reject(new DOMException('Media request cancelled', 'AbortError'));
    this.dispatch();
  }
  private trim() {
    let bytes = [...this.entries.values()].reduce((sum, item) => sum + item.bytes, 0);
    for (const [key, entry] of this.entries) {
      if (this.entries.size <= this.maxEntries && bytes <= this.maxBytes) break;
      if (entry.users !== 0) continue;
      bytes -= entry.bytes;
      this.remove(key, entry);
    }
  }
}

const caches = new WeakMap<IrisClient, MediaCache>();
export function mediaCache(client: IrisClient): MediaCache {
  let cache = caches.get(client);
  if (!cache) {
    cache = new MediaCache(client);
    caches.set(client, cache);
  }
  return cache;
}
export function mediaRevision(photo: Photo): string {
  return JSON.stringify([
    photo.path,
    photo.mtime,
    photo.size_bytes,
    photo.missing,
    photo.quarantined,
  ]);
}

export function previewNeighbors(photos: Photo[], focusedId: number | null): Photo[] {
  const index = photos.findIndex((photo) => photo.id === focusedId);
  if (index < 0) return [];
  return [photos[index + 1], photos[index - 1]].filter(
    (photo): photo is Photo => !!photo && !photo.missing && !photo.quarantined
  );
}
