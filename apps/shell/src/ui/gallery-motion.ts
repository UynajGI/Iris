import { useEffect, useLayoutEffect, useRef, type RefObject } from 'react';

export const GALLERY_MIN = 140;
export const GALLERY_LOUPE = 660;
type Rect = { left: number; top: number; width: number; height: number };
type Snapshot = { id: number; src: string; aspect: number; rect: Rect };
const reduced = () => window.matchMedia('(prefers-reduced-motion: reduce)').matches;
const easing = 'cubic-bezier(0.16, 1, 0.3, 1)';

/** Actual pixels inside an object-fit:contain frame (also handles portrait photos). */
export function containRect(frame: Rect, aspect: number): Rect {
  const width = Math.min(frame.width, frame.height * aspect), height = width / aspect;
  return { left: frame.left + (frame.width - width) / 2, top: frame.top + (frame.height - height) / 2, width, height };
}

/** Only visible tiles animate. Reads are batched before writes; rapid gestures
 * retarget from the currently painted positions, never queue old transitions. */
export function useGalleryMotion(root: RefObject<HTMLElement | null>, view: string, tileSize: number, identity: string) {
  const tiles = useRef<Map<string, Rect> | null>(null);
  const pending = useRef<Snapshot | null>(null);
  const overlay = useRef<HTMLImageElement | null>(null);
  const animations = useRef<Animation[]>([]);
  const cleanup = useRef<(() => void) | null>(null);
  const scrollTop = useRef(0);
  const cancel = () => {
    cleanup.current?.(); cleanup.current = null;
    animations.current.forEach(animation => animation.cancel()); animations.current = [];
    overlay.current?.remove(); overlay.current = null;
  };
  const captureTiles = () => {
    if (reduced()) return;
    const grid = root.current?.querySelector<HTMLElement>('.grid');
    if (!grid) return;
    const bounds = grid.getBoundingClientRect();
    const before = new Map<string, Rect>();
    grid.querySelectorAll<HTMLElement>('.cell').forEach(node => {
      const rect = node.getBoundingClientRect();
      if (rect.bottom > bounds.top && rect.top < bounds.bottom) before.set(node.dataset.photoId!, rect);
    });
    cancel(); tiles.current = before;
  };
  const capturePhoto = (id: number) => {
    const canvas = root.current?.querySelector<HTMLElement>('.canvas');
    const image = overlay.current ?? canvas?.querySelector<HTMLImageElement>(view === 'grid' ? `.cell[data-photo-id="${id}"] img` : '.zoom-image');
    const grid = canvas?.querySelector<HTMLElement>('.grid');
    if (grid) scrollTop.current = grid.scrollTop;
    const snapshot = image?.naturalWidth ? {
      id, src: image.currentSrc || image.src, aspect: image.naturalWidth / image.naturalHeight,
      rect: containRect(image.getBoundingClientRect(), image.naturalWidth / image.naturalHeight),
    } : null;
    cancel(); tiles.current = null; pending.current = null;
    if (!canvas || !snapshot || reduced()) return;
    pending.current = snapshot;
    const clone = document.createElement('img');
    clone.src = snapshot.src; clone.alt = ''; clone.setAttribute('aria-hidden', 'true');
    clone.className = 'gallery-flight';
    const bounds = canvas.getBoundingClientRect();
    Object.assign(clone.style, { left: `${snapshot.rect.left - bounds.left}px`, top: `${snapshot.rect.top - bounds.top}px`, width: `${snapshot.rect.width}px`, height: `${snapshot.rect.height}px` });
    canvas.append(clone); overlay.current = clone;
  };

  useLayoutEffect(() => {
    const canvas = root.current?.querySelector<HTMLElement>('.canvas');
    if (!canvas) { cancel(); pending.current = null; tiles.current = null; return; }
    const grid = canvas.querySelector<HTMLElement>('.grid');
    const snapshot = pending.current;
    pending.current = null;
    if (snapshot && overlay.current) {
      const clone = overlay.current;
      let target: Rect | undefined;
      if (grid) {
        grid.scrollTop = scrollTop.current;
        const cell = grid.querySelector<HTMLElement>(`.cell[data-photo-id="${snapshot.id}"]`);
        cell?.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'instant' });
        const frame = cell?.querySelector('.cell-photo')?.getBoundingClientRect();
        if (frame) target = containRect(frame, snapshot.aspect);
      } else {
        const stage = canvas.querySelector('.zoom-stage')?.getBoundingClientRect();
        if (stage) {
          const gutter = stage.width > 600 ? 56 : 24;
          target = containRect({ left: stage.left + gutter, top: stage.top + gutter / 2, width: Math.max(1, stage.width - gutter * 2), height: Math.max(1, stage.height - gutter) }, snapshot.aspect);
        }
      }
      if (!target || reduced()) { cancel(); return; }
      const targetImage = () => canvas.querySelector<HTMLImageElement>(grid ? `.cell[data-photo-id="${snapshot.id}"] img` : '.zoom-image');
      const destination = grid?.querySelector<HTMLElement>(`.cell[data-photo-id="${snapshot.id}"] .cell-photo`) ?? canvas.querySelector<HTMLElement>('.zoom-stage');
      destination?.classList.add('gallery-arriving');
      const animation = clone.animate([
        { transform: 'translate(0, 0) scale(1)' },
        { transform: `translate(${target.left - snapshot.rect.left}px, ${target.top - snapshot.rect.top}px) scale(${target.width / snapshot.rect.width})` },
      ], { duration: 300, easing, fill: 'forwards' });
      animations.current.push(animation);
      let finished = false;
      const reveal = () => {
        destination?.classList.remove('gallery-arriving');
        clone.remove(); if (overlay.current === clone) overlay.current = null;
      };
      const onLoad = () => { if (finished && targetImage()?.complete && targetImage()?.naturalWidth) reveal(); };
      canvas.addEventListener('load', onLoad, true);
      // A failed preview must reveal its real fallback, not strand a frozen thumbnail.
      const timeout = window.setTimeout(reveal, 1800);
      cleanup.current = () => { window.clearTimeout(timeout); canvas.removeEventListener('load', onLoad, true); reveal(); };
      animation.onfinish = () => { finished = true; onLoad(); };
    } else if (tiles.current && grid) {
      const before = tiles.current; tiles.current = null;
      const bounds = grid.getBoundingClientRect();
      const changes = Array.from(grid.querySelectorAll<HTMLElement>('.cell')).map(node => ({ node, old: before.get(node.dataset.photoId!), next: node.getBoundingClientRect() }));
      for (const { node, old, next } of changes) {
        if (!old || next.bottom < bounds.top || next.top > bounds.bottom || !next.width || reduced()) continue;
        animations.current.push(node.animate([
          { transform: `translate(${old.left - next.left}px, ${old.top - next.top}px) scale(${old.width / next.width}, ${old.height / next.height})` },
          { transform: 'none' },
        ], { duration: 200, easing }));
      }
    }
  }, [view, tileSize]);
  useEffect(() => () => { cancel(); pending.current = null; tiles.current = null; }, [identity]);
  return { captureTiles, capturePhoto };
}
