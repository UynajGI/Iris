import { useLayoutEffect, useRef, useState } from 'react';
import type { IrisClient } from '../../client.js';
import type { EditDocument, EditRecord } from '../../editing-client.js';
import {
  clampNativeCenter,
  nativeViewport,
  nativeImageOrigin,
  type RegionCenter,
} from '../../editing-region.js';
import { Button } from '../components.js';
import { useNativeRegion } from './useNativeRegion.js';

export function NativeDetailCanvas({
  client,
  record,
  document,
  preview,
  filename,
}: {
  client: IrisClient;
  record: EditRecord;
  document: EditDocument;
  preview: string | null;
  filename: string;
}) {
  const stage = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0, ratio: window.devicePixelRatio || 1 });
  const [center, setCenter] = useState<RegionCenter>([0.5, 0.5]);
  const [full, setFull] = useState({
    width: document.source.width,
    height: document.source.height,
  });
  const [loaded, setLoaded] = useState<string | null>(null);
  const [decodeError, setDecodeError] = useState<string | null>(null);
  const gesture = useRef<{ id: number; x: number; y: number; center: RegionCenter } | null>(null);
  useLayoutEffect(() => {
    if (!stage.current) return;
    const measure = () => {
      const bounds = stage.current!.getBoundingClientRect();
      setSize({ width: bounds.width, height: bounds.height, ratio: window.devicePixelRatio || 1 });
    };
    const observer = new ResizeObserver(measure);
    observer.observe(stage.current);
    window.addEventListener('resize', measure);
    return () => {
      observer.disconnect();
      window.removeEventListener('resize', measure);
    };
  }, []);
  const viewport = nativeViewport(size, size.ratio, center);
  const bounded = clampNativeCenter(center, full, viewport);
  const native = useNativeRegion(
    client,
    record,
    document,
    size.width && size.height ? { ...viewport, center: bounded } : null
  );
  const result = native.result;
  const region = result?.region;
  const ready = result && loaded === result.url;
  const error = native.error || (result?.url === decodeError ? '细节图加载失败' : null);
  const dimensions = region ? { width: region.fullWidth, height: region.fullHeight } : full;
  const actualCenter = clampNativeCenter(bounded, dimensions, viewport);
  const cssWidth = viewport.width / size.ratio,
    cssHeight = viewport.height / size.ratio;
  const move = (x: number, y: number, origin = actualCenter) =>
    setCenter(
      clampNativeCenter(
        [
          origin[0] - (x * size.ratio) / dimensions.width,
          origin[1] - (y * size.ratio) / dimensions.height,
        ],
        dimensions,
        viewport
      )
    );
  const navScale = Math.min(128 / dimensions.width, 100 / dimensions.height);
  return (
    <div
      ref={stage}
      className="grade-native-stage"
      tabIndex={0}
      aria-label="原尺寸细节，拖动或使用方向键移动"
      onKeyDown={(event) => {
        const steps: Record<string, [number, number]> = {
          ArrowLeft: [32, 0],
          ArrowRight: [-32, 0],
          ArrowUp: [0, 32],
          ArrowDown: [0, -32],
        };
        const step = steps[event.key];
        if (!step || event.target !== event.currentTarget) return;
        event.preventDefault();
        move(step[0], step[1]);
      }}
    >
      <div
        className="grade-native-viewport"
        style={{ width: cssWidth, height: cssHeight }}
        onPointerDown={(event) => {
          if (event.button !== 0) return;
          stage.current?.focus({ preventScroll: true });
          gesture.current = {
            id: event.pointerId,
            x: event.clientX,
            y: event.clientY,
            center: actualCenter,
          };
          event.currentTarget.setPointerCapture(event.pointerId);
        }}
        onPointerMove={(event) => {
          const drag = gesture.current;
          if (drag?.id === event.pointerId)
            move(event.clientX - drag.x, event.clientY - drag.y, drag.center);
        }}
        onPointerUp={() => {
          gesture.current = null;
        }}
        onPointerCancel={() => {
          gesture.current = null;
        }}
      >
        <div
          className="grade-native-image"
          style={{
            width: dimensions.width / size.ratio,
            height: dimensions.height / size.ratio,
            ...nativeImageOrigin(dimensions, viewport, actualCenter, size.ratio, region),
          }}
        >
          {preview && <img src={preview} alt={`${filename}，等待原尺寸细节`} draggable={false} />}
          {result && region && (
            <img
              className="grade-native-tile"
              src={result.url}
              alt={`${filename}，原尺寸细节`}
              style={{
                left: region.x / size.ratio,
                top: region.y / size.ratio,
                width: region.width / size.ratio,
                height: region.height / size.ratio,
                visibility: error ? 'hidden' : 'visible',
              }}
              draggable={false}
              onLoad={(event) => {
                if (
                  event.currentTarget.naturalWidth !== region.width ||
                  event.currentTarget.naturalHeight !== region.height
                ) {
                  setDecodeError(result.url);
                  return;
                }
                setFull({ width: region.fullWidth, height: region.fullHeight });
                setLoaded(result.url);
              }}
              onError={() => setDecodeError(result.url)}
            />
          )}
        </div>
      </div>
      <NativeDetailStatus ready={!!ready} error={error} onRetry={native.retry} />
      {preview && (
        <button
          type="button"
          className="grade-native-navigator"
          aria-label="定位细节区域"
          style={{ width: dimensions.width * navScale, height: dimensions.height * navScale }}
          onPointerDown={(event) => {
            event.preventDefault();
            const rect = event.currentTarget.getBoundingClientRect();
            setCenter(
              clampNativeCenter(
                [
                  (event.clientX - rect.left) / rect.width,
                  (event.clientY - rect.top) / rect.height,
                ],
                dimensions,
                viewport
              )
            );
            stage.current?.focus({ preventScroll: true });
          }}
          onClick={(event) => {
            if (event.detail === 0) {
              setCenter([0.5, 0.5]);
              stage.current?.focus({ preventScroll: true });
            }
          }}
        >
          <img src={preview} alt="" draggable={false} />
          <span
            style={{
              left:
                Math.max(0, actualCenter[0] - viewport.width / dimensions.width / 2) * 100 + '%',
              top:
                Math.max(0, actualCenter[1] - viewport.height / dimensions.height / 2) * 100 + '%',
              width: Math.min(100, (viewport.width / dimensions.width) * 100) + '%',
              height: Math.min(100, (viewport.height / dimensions.height) * 100) + '%',
            }}
          />
        </button>
      )}
    </div>
  );
}

function NativeDetailStatus({
  ready,
  error,
  onRetry,
}: {
  ready: boolean;
  error: string | null;
  onRetry(): void;
}) {
  if (!ready && !error) return null;
  return (
    <div className="grade-native-state" role="status">
      {error ?? (ready ? '1:1 · 原尺寸' : '')}
      {error && (
        <Button className="quiet" onClick={onRetry}>
          重试
        </Button>
      )}
    </div>
  );
}
