/** One render at a time; intermediate input is coalesced without starving display updates. */
export class EditingPreviewQueue<Input, Result> {
  private pending: { input: Input; version: number } | null = null;
  private active: { input: Input; controller: AbortController } | null = null;
  private version = 0;
  private disposed = false;
  constructor(
    private readonly handlers: {
      render(input: Input, signal: AbortSignal): Promise<Result>;
      result(result: Result, input: Input): void;
      error(error: unknown, input: Input): void;
      busy(value: boolean): void;
      cancel(input: Input): void;
    }
  ) {}
  push(input: Input, interrupt = false) {
    if (this.disposed) return;
    this.pending = { input, version: ++this.version };
    if (interrupt && this.active) {
      this.active.controller.abort();
      this.handlers.cancel(this.active.input);
    }
    if (!this.active) void this.drain();
  }
  dispose() {
    this.disposed = true;
    this.pending = null;
    if (this.active) {
      this.active.controller.abort();
      this.handlers.cancel(this.active.input);
    }
  }
  private async drain() {
    this.handlers.busy(true);
    while (this.pending && !this.disposed) {
      const item = this.pending;
      this.pending = null;
      const controller = new AbortController();
      this.active = { input: item.input, controller };
      try {
        const result = await this.handlers.render(item.input, controller.signal);
        if (!this.disposed && !controller.signal.aborted) this.handlers.result(result, item.input);
      } catch (error) {
        if (!this.disposed && !controller.signal.aborted && item.version === this.version)
          this.handlers.error(error, item.input);
      }
      this.active = null;
    }
    if (!this.disposed) this.handlers.busy(false);
  }
}
