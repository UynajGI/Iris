import { useCallback, useEffect, useLayoutEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react';
import type { IrisClient } from '../client.js';
import type { Photo } from '../types.js';
import { usePhotoUrl } from '../react.js';
import { Icon } from './Icon.js';

const MIN_SCALE_FACTOR = 1;
const MAX_ZOOM = 8;
// Webviews decode these source formats natively; others stay on the JPEG preview.
const browserDecodable = new Set(['jpeg', 'jpg', 'png', 'webp']);

type View = { zoom: number; x: number; y: number };
export type ZoomApi = { fit(): void; actual(): void; zoomBy(factor: number): void };

/** Pan/zoom viewer: preview first, original pixels once magnified past 100%. */
export function ZoomViewer({ client, photo, onZoomChange, apiRef }: {
  client: IrisClient; photo: Photo; onZoomChange?(percent: number, fitted: boolean): void; apiRef?: { current: ZoomApi | null };
}) {
  const stage = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [view, setView] = useState<View>({ zoom: 0, x: 0, y: 0 });
  const [dragging, setDragging] = useState(false);
  const drag = useRef<{ id: number; x: number; y: number; vx: number; vy: number } | null>(null);
  const wantsOriginal = view.zoom > 0 && browserDecodable.has(photo.format.toLowerCase());
  const [originalRequested, setOriginalRequested] = useState(false);
  const preview = usePhotoUrl(client, photo.id, 'preview');
  const original = usePhotoUrl(client, originalRequested ? photo.id : null, 'original');
  const [loaded, setLoaded] = useState<string | null>(null);
  const [originalReady, setOriginalReady] = useState(false);
  const [previewAspect, setPreviewAspect] = useState<number | null>(null);

  useEffect(() => { setView({ zoom: 0, x: 0, y: 0 }); setOriginalRequested(false); setOriginalReady(false); setPreviewAspect(null); }, [photo.id]);
  useLayoutEffect(() => {
    const node = stage.current;
    if (!node) return;
    const observer = new ResizeObserver(([entry]) => setSize({ width: entry!.contentRect.width, height: entry!.contentRect.height }));
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  // Stored dimensions may predate EXIF orientation; the oriented preview decides the axis.
  const rotated = previewAspect !== null && (previewAspect > 1) !== (photo.width > photo.height) && photo.width !== photo.height;
  const width = Math.max(1, rotated ? photo.height : photo.width), height = Math.max(1, rotated ? photo.width : photo.height);
  const gutter = size.width > 600 ? 56 : 24;
  const fitScale = size.width && size.height ? Math.min(Math.max(1, size.width - gutter * 2) / width, Math.max(1, size.height - gutter) / height, MIN_SCALE_FACTOR) : 1;
  const scale = view.zoom === 0 ? fitScale : view.zoom;
  const clamp = useCallback((next: View): View => {
    const s = next.zoom === 0 ? fitScale : next.zoom;
    const overflowX = Math.max(0, (width * s - size.width) / 2), overflowY = Math.max(0, (height * s - size.height) / 2);
    return { zoom: next.zoom, x: Math.min(overflowX, Math.max(-overflowX, next.x)), y: Math.min(overflowY, Math.max(-overflowY, next.y)) };
  }, [fitScale, width, height, size.width, size.height]);

  const zoomAt = useCallback((target: number, clientX?: number, clientY?: number) => {
    setView(current => {
      const from = current.zoom === 0 ? fitScale : current.zoom;
      const to = Math.min(MAX_ZOOM, Math.max(fitScale, target));
      if (to <= fitScale + 1e-4) return { zoom: 0, x: 0, y: 0 };
      const rect = stage.current?.getBoundingClientRect();
      const px = rect && clientX !== undefined ? clientX - rect.left - rect.width / 2 : 0;
      const py = rect && clientY !== undefined ? clientY - rect.top - rect.height / 2 : 0;
      // Keep the image point under the pointer stationary while scaling.
      const ratio = to / from;
      return clamp({ zoom: to, x: px - (px - current.x) * ratio, y: py - (py - current.y) * ratio });
    });
  }, [clamp, fitScale]);

  useEffect(() => { if (wantsOriginal && view.zoom > fitScale * 1.05) setOriginalRequested(true); }, [wantsOriginal, view.zoom, fitScale]);
  useEffect(() => { onZoomChange?.(Math.round(scale * 100), view.zoom === 0); }, [scale, view.zoom, onZoomChange]);
  useEffect(() => {
    if (!apiRef) return;
    apiRef.current = {
      fit: () => setView({ zoom: 0, x: 0, y: 0 }),
      actual: () => zoomAt(1),
      zoomBy: factor => zoomAt((view.zoom === 0 ? fitScale : view.zoom) * factor),
    };
    return () => { apiRef.current = null; };
  }, [apiRef, zoomAt, view.zoom, fitScale]);
  useEffect(() => {
    const node = stage.current;
    if (!node) return;
    // Non-passive listener so the page never scrolls while zooming the photo.
    const wheel = (event: WheelEvent) => {
      event.preventDefault();
      const delta = event.deltaMode === 1 ? event.deltaY * 16 : event.deltaY;
      setView(current => {
        const from = current.zoom === 0 ? fitScale : current.zoom;
        const to = Math.min(MAX_ZOOM, Math.max(fitScale, from * Math.exp(-delta * (event.ctrlKey ? 0.01 : 0.0018))));
        if (to <= fitScale + 1e-4) return { zoom: 0, x: 0, y: 0 };
        const rect = node.getBoundingClientRect();
        const px = event.clientX - rect.left - rect.width / 2, py = event.clientY - rect.top - rect.height / 2;
        const ratio = to / from;
        return clamp({ zoom: to, x: px - (px - current.x) * ratio, y: py - (py - current.y) * ratio });
      });
    };
    node.addEventListener('wheel', wheel, { passive: false });
    return () => node.removeEventListener('wheel', wheel);
  }, [clamp, fitScale]);
  useEffect(() => { setView(current => clamp(current)); }, [clamp]);

  const zoomed = view.zoom !== 0;
  const pointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (!zoomed || event.button !== 0) return;
    drag.current = { id: event.pointerId, x: event.clientX, y: event.clientY, vx: view.x, vy: view.y };
    event.currentTarget.setPointerCapture(event.pointerId);
    setDragging(true);
  };
  const pointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    const start = drag.current;
    if (!start || start.id !== event.pointerId) return;
    setView(current => clamp({ zoom: current.zoom, x: start.vx + event.clientX - start.x, y: start.vy + event.clientY - start.y }));
  };
  const pointerUp = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (drag.current?.id !== event.pointerId) return;
    drag.current = null;
    setDragging(false);
  };

  const showOriginal = originalRequested && original.url && originalReady;
  const source = showOriginal ? original.url : preview.url;
  const failed = !!preview.error && !original.url;
  return <div ref={stage} className={`zoom-stage ${zoomed ? 'zoomed' : ''} ${dragging ? 'dragging' : ''}`}
    onPointerDown={pointerDown} onPointerMove={pointerMove} onPointerUp={pointerUp} onPointerCancel={pointerUp}
    onDoubleClick={event => { if (zoomed) setView({ zoom: 0, x: 0, y: 0 }); else zoomAt(Math.max(1, fitScale * 2), event.clientX, event.clientY); }}>
    {source && <img className={`zoom-image ${loaded === source ? 'loaded' : 'loading'}`} src={source} alt={photo.filename} draggable={false}
      onLoad={event => { setLoaded(source); if (source === preview.url) setPreviewAspect(event.currentTarget.naturalWidth / Math.max(1, event.currentTarget.naturalHeight)); }}
      style={{ width: width * scale, height: height * scale, transform: `translate(-50%, -50%) translate(${view.x}px, ${view.y}px)`, imageRendering: scale >= 2 ? 'pixelated' : 'auto' }} />}
    {original.url && !originalReady && <img className="zoom-preload" src={original.url} alt="" aria-hidden="true" onLoad={() => setOriginalReady(true)} />}
    {!source && !failed && <span className="zoom-loading" role="status" aria-label={`${photo.filename}：正在载入`} />}
    {failed && <span className="zoom-failed" role="img" aria-label={`${photo.filename}：照片暂不可用`}><Icon name="image" />照片暂不可用</span>}
    {originalRequested && !showOriginal && !original.error && <span className="zoom-badge" role="status">正在载入原图</span>}
    {zoomed && size.width > 0 && <Minimap photoWidth={width} photoHeight={height} scale={scale} view={view} stage={size} url={preview.url} />}
  </div>;
}

function Minimap({ photoWidth, photoHeight, scale, view, stage, url }: { photoWidth: number; photoHeight: number; scale: number; view: View; stage: { width: number; height: number }; url: string | null }) {
  const box = 132;
  const ratio = Math.min(box / photoWidth, box / photoHeight);
  const w = photoWidth * ratio, h = photoHeight * ratio;
  const viewW = Math.min(1, stage.width / (photoWidth * scale)), viewH = Math.min(1, stage.height / (photoHeight * scale));
  const centerX = 0.5 - view.x / (photoWidth * scale), centerY = 0.5 - view.y / (photoHeight * scale);
  return <div className="minimap" aria-hidden="true" style={{ width: w, height: h }}>
    {url && <img src={url} alt="" draggable={false} />}
    <span className="minimap-view" style={{ left: `${(centerX - viewW / 2) * 100}%`, top: `${(centerY - viewH / 2) * 100}%`, width: `${viewW * 100}%`, height: `${viewH * 100}%` }} />
  </div>;
}
