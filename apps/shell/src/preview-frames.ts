/** Keep a decoded frame visible until its replacement is ready; discard late decodes. */
export class PreviewFrames {
  private generation = 0;
  private current: string | null = null;
  constructor(
    private decode = async (url: string) => {
      const image = new Image();
      image.src = url;
      await image.decode();
    }
  ) {}
  async show(blob: Blob, accept: (url: string) => boolean, fail: (error: unknown) => void) {
    const generation = ++this.generation;
    const url = URL.createObjectURL(blob);
    try {
      await this.decode(url);
      if (generation !== this.generation || !accept(url)) {
        URL.revokeObjectURL(url);
        return;
      }
      const previous = this.current;
      this.current = url;
      if (previous) URL.revokeObjectURL(previous);
    } catch (error) {
      URL.revokeObjectURL(url);
      if (generation === this.generation) fail(error);
    }
  }
  dispose() {
    ++this.generation;
    if (this.current) URL.revokeObjectURL(this.current);
    this.current = null;
  }
}
