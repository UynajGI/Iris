import type { MarkRequest, Photo } from './types.js';

type Fields = Partial<Pick<Photo, 'decision' | 'rating' | 'color_label'>>;
type Key = keyof Fields;
export interface PendingMark {
  request: MarkRequest;
  before: Map<number, Fields>;
}

/** Ordered overlays keep rapid edits visible while the server serializes writes. */
export class OptimisticMarks {
  private entries: PendingMark[] = [];
  get pending(): boolean {
    return this.entries.length > 0;
  }
  clear(): void {
    this.entries = [];
  }
  add(request: MarkRequest, photos: Photo[]): PendingMark {
    const before = new Map<number, Fields>();
    for (const photo of photos) {
      if (!request.photo_ids.includes(photo.id) || before.has(photo.id)) continue;
      before.set(
        photo.id,
        Object.fromEntries(
          this.keys(request).map((key) => [key, photo[key] ?? (key === 'rating' ? 0 : 'none')])
        ) as Fields
      );
    }
    const entry = { request, before };
    this.entries.push(entry);
    return entry;
  }
  apply = (photo: Photo): Photo => {
    let next = photo;
    for (const { request } of this.entries) {
      if (request.photo_ids.includes(photo.id)) {
        const fields = Object.fromEntries(
          this.keys(request).map((key) => [key, request[key]])
        ) as Fields;
        next = { ...next, ...fields };
      }
    }
    return next;
  };
  settle(entry: PendingMark, failed: boolean): (photo: Photo) => Photo {
    const index = this.entries.indexOf(entry);
    if (index < 0) return (photo) => photo;
    if (failed) {
      // A later failed write must roll back to the last successful value, not
      // to the optimistic value of this failed predecessor.
      for (const [id, before] of entry.before)
        for (const key of this.keys(entry.request)) {
          const successor = this.entries
            .slice(index + 1)
            .find((next) => next.before.has(id) && key in next.request);
          if (successor) Object.assign(successor.before.get(id)!, { [key]: before[key] });
        }
    }
    this.entries.splice(index, 1);
    return (photo) =>
      this.apply(
        failed && entry.before.has(photo.id) ? { ...photo, ...entry.before.get(photo.id) } : photo
      );
  }
  private keys(request: MarkRequest): Key[] {
    return (['decision', 'rating', 'color_label'] as const).filter((key) => request[key] != null);
  }
}
