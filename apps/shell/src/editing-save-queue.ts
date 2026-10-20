// SPDX-License-Identifier: GPL-3.0-or-later

/** Navigation and autosave share one drain; edits arriving during a write follow it. */
export class EditingSaveQueue {
  private running: Promise<boolean> | null = null;

  flush(step: () => Promise<'clean' | 'pending' | 'failed'>): Promise<boolean> {
    if (this.running) return this.running;
    const run = async () => {
      for (;;) {
        const result = await step();
        if (result !== 'pending') return result === 'clean';
      }
    };
    const promise = run().finally(() => {
      if (this.running === promise) this.running = null;
    });
    this.running = promise;
    return promise;
  }
}
