import { useCallback, useEffect, useRef, useState, type RefObject } from 'react';
import type { ApplicationState, IrisStore } from '../store.js';
import type { Photo } from '../types.js';
import { useFullscreen } from './fullscreen.js';
import { GALLERY_LOUPE, GALLERY_MIN, useGalleryMotion } from './gallery-motion.js';
import { usePersisted } from './usePersisted.js';
import type { ZoomApi } from './ZoomViewer.js';

interface GalleryOptions {
  store: IrisStore; state: ApplicationState; page: string;
  workspace: RefObject<HTMLElement | null>; current: Photo | undefined;
}

/** Grid/loupe continuity, scale and wheel input share one controller. */
export function useGalleryView({ store, state, page, workspace, current }: GalleryOptions) {
  const group = state.groupSession;
  const stageRef = useRef<HTMLDivElement>(null);
  const zoomApi = useRef<ZoomApi | null>(null);
  const [zoomReadout, setZoomReadout] = useState({ percent: 100, fitted: true });
  const onZoomChange = useCallback((percent: number, fitted: boolean) => setZoomReadout({ percent, fitted }), []);
  const fullscreen = useFullscreen(stageRef);
  const [tileSize, setTileSize] = usePersisted<number>('iris:tile-size:v2', 210, value => value >= GALLERY_MIN && value < GALLERY_LOUPE);
  const view = group ? 'compare' : state.reviewOpen ? 'loupe' : 'grid';
  const galleryMotion = useGalleryMotion(workspace, view, tileSize, `${page}:${state.project?.id}`);
  const setView = (target: 'grid' | 'loupe') => {
    if (target !== view && current && !group) galleryMotion.capturePhoto(current.id);
    if (group) store.closeGroup();
    if (target === 'loupe') store.select([]);
    if ((target === 'loupe') !== state.reviewOpen) store.toggleReview();
  };
  const resizeGallery = (value: number) => {
    if (!current || group) return;
    if (value >= GALLERY_LOUPE) { setView('loupe'); return; }
    if (view === 'grid') galleryMotion.captureTiles(); else setView('grid');
    setTileSize(Math.max(GALLERY_MIN, Math.min(GALLERY_LOUPE - 1, value)));
  };
  const stepZoom = (factor: number) => {
    if (view === 'grid') resizeGallery(Math.round(tileSize * factor));
    else if (factor < 1 && zoomReadout.fitted) setView('grid');
    else zoomApi.current?.zoomBy(factor);
  };
  const galleryWheel = useRef<(event: WheelEvent) => void>(() => { });
  galleryWheel.current = event => {
    if (view !== 'grid' || !(event.ctrlKey || event.metaKey) || !current) return;
    event.preventDefault();
    const delta = event.deltaY * (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? 400 : 1);
    resizeGallery(Math.round(tileSize * Math.exp(-Math.max(-150, Math.min(150, delta)) * 0.006)));
  };
  useEffect(() => {
    const node = workspace.current?.querySelector('.canvas');
    if (!node) return;
    const wheel = (event: Event) => galleryWheel.current(event as WheelEvent);
    node.addEventListener('wheel', wheel, { passive: false });
    return () => node.removeEventListener('wheel', wheel);
  }, [page, state.project?.id]);
  const openLoupe = (photo: Photo) => {
    if (!state.reviewOpen && !group) galleryMotion.capturePhoto(photo.id);
    store.select([]); store.focus(photo.id);
    if (!state.reviewOpen) store.toggleReview();
  };
  return { stageRef, zoomApi, zoomReadout, onZoomChange, fullscreen, tileSize, view, setView, resizeGallery, stepZoom, openLoupe };
}
