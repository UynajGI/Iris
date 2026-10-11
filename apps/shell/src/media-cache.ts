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
};
export interface MediaLease {
  promise: Promise<Blob>;
  release(): void;
  invalidate(): void;
}

/** Cache compressed media, not decoded full-resolution bitmaps. Active leases own requests. */
export class MediaCache {
  private entries = new Map<string, Entry>();
  constructor(
    private source: Pick<IrisClient, 'media'>,
    private maxEntries = 24,
    private maxBytes = 32 * 1024 * 1024
  ) {}
  acquire(id: number, kind: Kind, revision = ''): MediaLease {
    const key = JSON.stringify([id, kind, revision]);
    let entry = this.entries.get(key);
    if (!entry) {
      entry = this.create(id, kind);
      this.entries.set(key, entry);
      const captured = entry;
      void entry.promise.then(
        (blob) => {
          captured.settled = true;
          captured.bytes = blob.size;
          if (captured.users === 0 && kind === 'original') this.remove(key, captured);
          this.trim();
        },
        () => this.remove(key, captured)
      );
    }
    this.entries.delete(key);
    this.entries.set(key, entry);
    entry.users++;
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
  private create(id: number, kind: Kind): Entry {
    const controller = new AbortController();
    return {
      controller,
      promise: this.source.media(id, kind, controller.signal),
      users: 0,
      bytes: 0,
      settled: false,
      kind,
    };
  }
  private remove(key: string, entry: Entry) {
    if (this.entries.get(key) !== entry) return;
    entry.controller.abort();
    this.entries.delete(key);
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
