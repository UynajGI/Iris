export interface UpdateError { code: string; message: string }
export interface UpdateInfo {
  version: string;
  current_version: string;
  notes: string | null;
  published_at_unix: number | null;
}
export interface UpdateStatus {
  state: 'not_configured' | 'idle' | 'checking' | 'up_to_date' | 'available' | 'downloading' | 'downloaded' | 'installing' | 'failed';
  current_version: string;
  update: UpdateInfo | null;
  downloaded_bytes: number;
  total_bytes: number | null;
  reason: string | null;
  error: UpdateError | null;
}
export interface UpdaterState { status: UpdateStatus | null; pending: boolean; error: UpdateError | null }
export interface NativeUpdaterBridge {
  core: { invoke<T>(command: string): Promise<T> };
  event: { listen<T>(event: string, callback: (event: { payload: T }) => void): Promise<() => void> };
}
type UpdateCommand = 'update_status' | 'check_for_update' | 'download_update' | 'install_update';

export class UpdateCommandError extends Error {
  constructor(readonly code: string, message: string) { super(message); this.name = 'UpdateCommandError'; }
}
function commandError(error: unknown): UpdateCommandError {
  if (typeof error === 'object' && error !== null && 'code' in error && 'message' in error
    && typeof error.code === 'string' && typeof error.message === 'string') return new UpdateCommandError(error.code, error.message);
  return new UpdateCommandError('native_error', error instanceof Error ? error.message : String(error));
}

/** Native updater state is independent of the daemon's project/session lifecycle. */
export class UpdaterStore {
  private state: UpdaterState = { status: null, pending: false, error: null };
  private listeners = new Set<() => void>();
  private generation = 0;
  private eventRevision = 0;
  private unlisten: (() => void) | undefined;
  private connection: Promise<void> | undefined;
  private queue: Promise<unknown> = Promise.resolve();
  constructor(private readonly bridge: NativeUpdaterBridge) {}
  getSnapshot = (): UpdaterState => this.state;
  subscribe = (listener: () => void): (() => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener); }; };
  private update(patch: Partial<UpdaterState>): void {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) listener();
  }
  /** Subscribe before the first read, so a download progress event cannot be missed. */
  connect = (): Promise<void> => {
    if (this.connection) return this.connection;
    const generation = this.generation;
    const attach = async () => {
      try {
        const unlisten = await this.bridge.event.listen<UpdateStatus>('updater:status', event => {
          if (generation !== this.generation) return;
          ++this.eventRevision;
          this.update({ status: event.payload, error: event.payload.error });
        });
        if (generation !== this.generation) { unlisten(); return; }
        this.unlisten = unlisten;
        await this.refresh();
      } catch (error) {
        if (generation === this.generation) {
          this.unlisten?.(); this.unlisten = undefined; this.connection = undefined;
          const failure = commandError(error);
          this.update({ error: { code: failure.code, message: failure.message } });
        }
        throw error;
      }
    };
    this.connection = attach();
    return this.connection;
  };
  private command(command: UpdateCommand): Promise<UpdateStatus | null> {
    const generation = this.generation;
    const next = this.queue.then(async () => {
      if (generation !== this.generation) return null;
      const revision = this.eventRevision;
      this.update({ pending: true, error: null });
      try {
        const status = await this.bridge.core.invoke<UpdateStatus>(command);
        if (generation === this.generation && revision === this.eventRevision) this.update({ status, error: status.error });
        return generation === this.generation ? this.state.status : null;
      } catch (error) {
        const failure = commandError(error);
        if (generation === this.generation && revision === this.eventRevision) this.update({ error: { code: failure.code, message: failure.message } });
        throw failure;
      } finally { if (generation === this.generation) this.update({ pending: false }); }
    });
    this.queue = next.catch(() => undefined);
    return next;
  }
  refresh = (): Promise<UpdateStatus | null> => this.command('update_status');
  check = (): Promise<UpdateStatus | null> => this.command('check_for_update');
  download = (): Promise<UpdateStatus | null> => this.command('download_update');
  /** Installation is only triggered by this explicit call; Windows may exit on success. */
  install = (): Promise<UpdateStatus | null> => this.command('install_update');
  dispose(): void {
    ++this.generation;
    this.unlisten?.(); this.unlisten = undefined; this.connection = undefined;
    this.update({ pending: false });
    this.listeners.clear();
  }
}
