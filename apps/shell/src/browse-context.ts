import type { IrisClient } from './client.js';
import type { BrowseContext } from './types.js';

export type BrowseTargets = Pick<
  BrowseContext,
  'focused_photo_id' | 'comparison_photo_ids' | 'nearby_photo_ids'
>;
const empty: BrowseTargets = {
  focused_photo_id: null,
  comparison_photo_ids: [],
  nearby_photo_ids: [],
};
let revision = 0;
const clientId = `shell-${globalThis.crypto.randomUUID()}`;

/** Coalesce rapid navigation; revisions also protect against reordered HTTP delivery. */
export class BrowseContextPublisher {
  private timer: ReturnType<typeof setTimeout> | undefined;
  private heartbeat: ReturnType<typeof setInterval> | undefined;
  private request: AbortController | undefined;
  private targets = empty;
  private key = '';
  private disposed = false;
  constructor(
    private client: IrisClient,
    private projectId: number
  ) {}
  update(targets: BrowseTargets) {
    if (this.disposed || JSON.stringify(targets) === this.key) return;
    this.targets = targets;
    this.key = JSON.stringify(targets);
    clearTimeout(this.timer);
    this.timer = setTimeout(() => this.publish(), 60);
    this.heartbeat ??= setInterval(() => this.publish(), 10_000);
  }
  private publish() {
    this.request?.abort();
    this.request = new AbortController();
    void this.client
      .browseContext(
        this.projectId,
        {
          ...this.targets,
          client_id: clientId,
          revision: ++revision,
        },
        this.request.signal
      )
      .catch(() => {
        /* Scheduling hints must not interrupt selection. */
      });
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    clearTimeout(this.timer);
    clearInterval(this.heartbeat);
    this.targets = empty;
    this.publish();
  }
}

export function browseTargets(
  focusedId: number | null,
  comparisons: number[],
  orderedIds: number[]
): BrowseTargets {
  const index = focusedId === null ? -1 : orderedIds.indexOf(focusedId);
  const nearby =
    index < 0 ? orderedIds.slice(0, 24) : orderedIds.slice(Math.max(0, index - 12), index + 13);
  return {
    focused_photo_id: focusedId,
    comparison_photo_ids: comparisons.slice(0, 32),
    nearby_photo_ids: nearby.filter((id) => id !== focusedId && !comparisons.includes(id)),
  };
}
