import { useEffect, useLayoutEffect, useRef, useState, type PointerEvent } from 'react';
import type { Photo } from '../../types.js';
import type { IrisClient } from '../../client.js';
import { usePhotoUrl } from '../../react.js';
import { mediaRevision } from '../../media-cache.js';
import { type BrushStroke, type MaskDefinition, type MaskLayer } from '../../editing-masks.js';
import type { BrushSettings } from './MaskPanel.js';
import { MaskOverlay } from './MaskOverlay.js';
import { WorkingMaskOverlay, type MaskPreviewSource } from './WorkingMaskOverlay.js';
import { needsWorkingMask } from '../../editing-mask-preview.js';
import { CropOverlay } from './CropOverlay.js';
import { ClippingOverlay } from './ClippingOverlay.js';
import { noClipping, type Clipping } from '../scope-pixels.js';
import { WhiteBalancePicker } from './WhiteBalancePicker.js';
import { fitPreview } from '../../editing-canvas.js';
import {
  maskPartAt,
  paintMaskPart,
  replaceMaskPart,
  type MaskPart,
} from '../../editing-mask-parts.js';

type Point = [number, number];
type Gesture = {
  id: number;
  start: Point;
  definition: MaskDefinition;
  points: Point[];
  primary: MaskDefinition;
  part: MaskPart;
};
function pointsIn(mask: MaskDefinition): number {
  if (mask.kind === 'brush')
    return mask.strokes.reduce((sum, stroke) => sum + stroke.points.length, 0);
  if (mask.kind === 'combine') return pointsIn(mask.left) + pointsIn(mask.right);
  if (mask.kind === 'invert' || mask.kind === 'opacity') return pointsIn(mask.mask);
  return 0;
}
function brushDefinition(gesture: Gesture, brush: BrushSettings): MaskDefinition {
  const stroke: BrushStroke = {
    points: gesture.points,
    radius: brush.radius,
    hardness: brush.hardness,
    flow: brush.flow,
  };
  return paintMaskPart(gesture.definition, gesture.part, stroke, brush.erase);
}
export function ColorCanvas({
  client,
  photo,
  preview,
  before = false,
  zoom,
  layer,
  selectedPart = 0,
  painting,
  brush,
  onMask,
  showMask = true,
  clipping = noClipping,
  maskPreview,
  crop,
  cropLocked,
  onCrop,
  neutralPicker,
}: {
  client: IrisClient;
  photo: Photo;
  preview: string | null;
  before?: boolean;
  zoom: number;
  layer: MaskLayer | undefined;
  selectedPart?: number;
  painting: boolean;
  brush: BrushSettings;
  onMask(definition: MaskDefinition): void;
  showMask?: boolean;
  clipping?: Clipping;
  maskPreview?: MaskPreviewSource | undefined;
  crop?: number[] | undefined;
  cropLocked?: boolean;
  onCrop?: ((crop: number[]) => void) | undefined;
  neutralPicker?:
    { width: number; height: number; onPick(point: Point): void; onCancel(): void } | undefined;
}) {
  const original = usePhotoUrl(client, photo.id, 'preview', mediaRevision(photo));
  const url = preview ?? original.url;
  const stage = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [natural, setNatural] = useState({ width: photo.width, height: photo.height });
  const [loadedUrl, setLoadedUrl] = useState<string | null>(null);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const panGesture = useRef<{ id: number; x: number; y: number; base: typeof pan } | null>(null);
  const gesture = useRef<Gesture | null>(null);
  const part = layer && maskPartAt(layer.definition, selectedPart);
  const primary = part?.definition;
  useEffect(() => {
    gesture.current = null;
  }, [layer?.id, selectedPart]);
  const isDrawing =
    painting && primary && ['brush', 'linear_gradient', 'radial_gradient'].includes(primary.kind);
  useLayoutEffect(() => {
    if (!stage.current) return;
    const observer = new ResizeObserver(([entry]) =>
      setSize({ width: entry!.contentRect.width, height: entry!.contentRect.height })
    );
    observer.observe(stage.current);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    setPan({ x: 0, y: 0 });
  }, [photo.id, zoom]);
  const { width, height } = fitPreview(natural, size);
  useEffect(() => {
    const limitX = Math.max(0, (width * zoom - size.width) / 2),
      limitY = Math.max(0, (height * zoom - size.height) / 2);
    setPan((value) => ({
      x: Math.max(-limitX, Math.min(limitX, value.x)),
      y: Math.max(-limitY, Math.min(limitY, value.y)),
    }));
  }, [width, height, zoom, size.width, size.height]);
  const position = (event: PointerEvent<SVGSVGElement>): Point => {
    const rect = event.currentTarget.getBoundingClientRect();
    return [
      Math.max(0, Math.min(1, (event.clientX - rect.left) / rect.width)),
      Math.max(0, Math.min(1, (event.clientY - rect.top) / rect.height)),
    ];
  };
  const updateGesture = (event: PointerEvent<SVGSVGElement>) => {
    const current = gesture.current;
    if (!current || current.id !== event.pointerId) return;
    const point = position(event);
    const base = current.primary;
    if (base.kind === 'brush') {
      const last = current.points.at(-1)!;
      if (
        Math.hypot(last[0] - point[0], last[1] - point[1]) < 0.002 ||
        pointsIn(current.definition) + current.points.length >= 4096
      )
        return;
      current.points = [...current.points, point];
      onMask(brushDefinition(current, brush));
    } else if (
      base.kind === 'linear_gradient' &&
      Math.hypot(current.start[0] - point[0], current.start[1] - point[1]) > 0.005
    ) {
      onMask(
        replaceMaskPart(current.definition, current.part.path, {
          kind: 'linear_gradient',
          start: current.start,
          end: point,
        })
      );
    } else if (base.kind === 'radial_gradient') {
      onMask(
        replaceMaskPart(current.definition, current.part.path, {
          ...base,
          center: current.start,
          radii: [
            Math.max(0.005, Math.abs(point[0] - current.start[0])),
            Math.max(0.005, Math.abs(point[1] - current.start[1])),
          ],
        })
      );
    }
  };
  return (
    <div
      ref={stage}
      className={`grade-canvas ${isDrawing ? 'drawing' : zoom > 1 ? 'panning' : ''}`}
      onPointerDown={(event) => {
        if (isDrawing || crop || neutralPicker || zoom === 1) return;
        panGesture.current = { id: event.pointerId, x: event.clientX, y: event.clientY, base: pan };
        event.currentTarget.setPointerCapture(event.pointerId);
      }}
      onPointerMove={(event) => {
        const move = panGesture.current;
        if (!move || move.id !== event.pointerId) return;
        const limitX = Math.max(0, (width * zoom - size.width) / 2),
          limitY = Math.max(0, (height * zoom - size.height) / 2);
        setPan({
          x: Math.max(-limitX, Math.min(limitX, move.base.x + event.clientX - move.x)),
          y: Math.max(-limitY, Math.min(limitY, move.base.y + event.clientY - move.y)),
        });
      }}
      onPointerUp={() => {
        panGesture.current = null;
      }}
      onPointerCancel={() => {
        panGesture.current = null;
      }}
    >
      {url ? (
        <div
          className="grade-canvas-image"
          style={{ width, height, transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})` }}
        >
          <img
            src={url}
            alt={`${photo.filename}${before ? '，原图' : preview ? '，调整后' : ''}`}
            draggable={false}
            onLoad={(event) => {
              setLoadedUrl(url);
              setNatural({
                width: event.currentTarget.naturalWidth,
                height: event.currentTarget.naturalHeight,
              });
            }}
          />
          {(clipping.shadows || clipping.highlights) && (
            <ClippingOverlay
              key={`${photo.id}:${natural.width}:${natural.height}`}
              url={url}
              clipping={clipping}
            />
          )}
          {neutralPicker && preview && loadedUrl === preview && (
            <WhiteBalancePicker {...neutralPicker} />
          )}
          {crop && onCrop && (
            <CropOverlay
              crop={crop}
              width={width}
              height={height}
              locked={!!cropLocked}
              onChange={onCrop}
            />
          )}
          {layer &&
            showMask &&
            !before &&
            !crop &&
            (needsWorkingMask(layer.definition) && maskPreview ? (
              <WorkingMaskOverlay
                client={client}
                maskId={layer.id}
                source={{
                  ...maskPreview,
                  displayed: maskPreview.displayed,
                }}
              />
            ) : (
              <MaskOverlay
                definition={layer.definition}
                width={natural.width}
                height={natural.height}
              />
            ))}
          {isDrawing && layer && (
            <svg
              className="grade-mask-guides"
              viewBox="0 0 1 1"
              preserveAspectRatio="none"
              aria-label={`绘制${layer.name}`}
              onPointerDown={(event) => {
                if (!part || pointsIn(layer.definition) >= 4096) return;
                const point = position(event);
                gesture.current = {
                  id: event.pointerId,
                  start: point,
                  definition: layer.definition,
                  points: [point],
                  primary: part.definition,
                  part,
                };
                event.currentTarget.setPointerCapture(event.pointerId);
                if (gesture.current.primary.kind === 'brush')
                  onMask(brushDefinition(gesture.current, brush));
              }}
              onPointerMove={updateGesture}
              onPointerUp={(event) => {
                updateGesture(event);
                gesture.current = null;
              }}
              onPointerCancel={() => {
                gesture.current = null;
              }}
            >
              {primary?.kind === 'linear_gradient' && (
                <g>
                  <line
                    x1={primary.start[0]}
                    y1={primary.start[1]}
                    x2={primary.end[0]}
                    y2={primary.end[1]}
                  />
                  <ellipse
                    cx={primary.start[0]}
                    cy={primary.start[1]}
                    rx={4 / width}
                    ry={4 / height}
                  />
                  <ellipse cx={primary.end[0]} cy={primary.end[1]} rx={4 / width} ry={4 / height} />
                </g>
              )}
              {primary?.kind === 'radial_gradient' && (
                <g>
                  <ellipse
                    cx={primary.center[0]}
                    cy={primary.center[1]}
                    rx={primary.radii[0]}
                    ry={primary.radii[1]}
                  />
                  <ellipse
                    className="grade-mask-feather"
                    cx={primary.center[0]}
                    cy={primary.center[1]}
                    rx={primary.radii[0]! * primary.hardness}
                    ry={primary.radii[1]! * primary.hardness}
                  />
                </g>
              )}
              {primary?.kind === 'brush' &&
                primary.strokes.map((stroke, index) => (
                  <polyline
                    className="grade-brush-guide"
                    key={index}
                    points={stroke.points.map((point) => point.join(',')).join(' ')}
                  />
                ))}
            </svg>
          )}
        </div>
      ) : (
        <span role="status">{original.error ? '照片预览未完成' : ''}</span>
      )}
    </div>
  );
}
