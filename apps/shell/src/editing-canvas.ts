/** Proxy resolution must never change the on-screen image size while dragging. */
export function fitPreview(
  natural: { width: number; height: number },
  viewport: { width: number; height: number }
) {
  const scale = Math.min(
    Math.max(1, viewport.width - 40) / Math.max(1, natural.width),
    Math.max(1, viewport.height - 40) / Math.max(1, natural.height)
  );
  return { width: Math.max(1, natural.width * scale), height: Math.max(1, natural.height * scale) };
}
