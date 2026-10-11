import { useId, useState } from 'react';
import type { IrisClient } from '../client.js';
import type { Photo } from '../types.js';
import { usePhotoUrl } from '../react.js';
import { mediaRevision } from '../media-cache.js';
import {
  faceRegions,
  originalDetailAvailable,
  originalDetailPhotoId,
  scaleFaceRegion,
  type FaceRegion,
} from '../selection-readout.js';

export function FaceDetail({
  client,
  photo,
  active,
}: {
  client: IrisClient;
  photo: Photo;
  active: boolean;
}) {
  const regions = faceRegions(photo);
  const [selected, setSelected] = useState(0);
  const region = regions[Math.min(selected, regions.length - 1)];
  const {
    preview,
    original,
    showOriginal,
    originalFailed,
    source,
    setFailed,
    setReady,
    status,
    ready,
  } = useDetailMedia(client, photo, region, active);
  if (!region) return <p className="detail-empty">没有可复核的人脸区域，请切回整图观察</p>;
  return (
    <div className="face-detail">
      <label className="face-detail-picker">
        人脸区域
        <select
          aria-label={`${photo.filename} 的人脸区域`}
          value={Math.min(selected, regions.length - 1)}
          onChange={(event) => setSelected(Number(event.target.value))}
        >
          {regions.map((item, index) => (
            <option key={item.index} value={index}>
              人脸 {index + 1}（从左到右）
            </option>
          ))}
        </select>
      </label>
      <FaceCrop
        source={source}
        region={showOriginal && ready ? scaleFaceRegion(region, ready.width, ready.height) : region}
        name={`${photo.filename} 的人脸 ${selected + 1} 细节`}
        failed={!source && !!preview.error}
        invalidate={(showOriginal ? original : preview).invalidate}
      />
      {original.url && !showOriginal && !originalFailed && (
        <img
          className="zoom-preload"
          src={original.url}
          alt=""
          onLoad={(event) => {
            const aspect = event.currentTarget.naturalWidth / event.currentTarget.naturalHeight;
            if (Math.abs(aspect / (region.width / region.height) - 1) > 0.02)
              setFailed(original.url);
            else
              setReady({
                url: original.url!,
                width: event.currentTarget.naturalWidth,
                height: event.currentTarget.naturalHeight,
              });
          }}
          onError={() => setFailed(original.url)}
        />
      )}
      <p className="detail-source" role="status">
        {status}
      </p>
    </div>
  );
}

function useDetailMedia(
  client: IrisClient,
  photo: Photo,
  region: FaceRegion | undefined,
  active: boolean
) {
  const revision = mediaRevision(photo);
  const preview = usePhotoUrl(client, region ? photo.id : null, 'preview', revision);
  const supported = originalDetailAvailable(photo);
  const original = usePhotoUrl(
    client,
    originalDetailPhotoId(photo, active && !!region),
    'original',
    revision
  );
  const [ready, setReady] = useState<{ url: string; width: number; height: number } | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const originalFailed = !!original.error || (!!original.url && failed === original.url);
  const showOriginal = active && !!original.url && ready?.url === original.url && !originalFailed;
  const source = showOriginal ? original.url : preview.url;
  const status = detailStatus(showOriginal, active, supported, originalFailed);
  return {
    preview,
    original,
    showOriginal,
    originalFailed,
    source,
    setFailed,
    setReady,
    status,
    ready,
  };
}

function detailStatus(showOriginal: boolean, active: boolean, supported: boolean, failed: boolean) {
  if (showOriginal) return '原图局部 · 按脸部大小对齐';
  if (!active) return '预览局部 · 选中照片可查看原图';
  if (!supported) return '预览局部 · 此格式或尺寸暂不读取原图细节';
  return failed ? '原图暂不可用 · 当前为预览局部' : '预览局部 · 正在读取原图';
}

function FaceCrop({
  source,
  region,
  name,
  failed,
  invalidate,
}: {
  source: string | null;
  region: FaceRegion;
  name: string;
  failed: boolean;
  invalidate(): void;
}) {
  const [failedSource, setFailedSource] = useState<string | null>(null);
  const clipId = useId();
  const unavailable = failed || (!!source && source === failedSource);
  return (
    <div className="face-detail-frame">
      {source && !unavailable ? (
        <svg
          viewBox={`${region.x} ${region.y} ${region.w} ${region.h}`}
          role="img"
          aria-label={name}
          preserveAspectRatio="xMidYMid meet"
          overflow="hidden"
        >
          <defs>
            <clipPath id={clipId}>
              <rect x={region.x} y={region.y} width={region.w} height={region.h} />
            </clipPath>
          </defs>
          <image
            href={source}
            width={region.width}
            height={region.height}
            preserveAspectRatio="none"
            clipPath={`url(#${clipId})`}
            onError={() => {
              invalidate();
              setFailedSource(source);
            }}
          />
        </svg>
      ) : (
        <p role="status">{unavailable ? '图像暂不可用，请重新打开比较' : '正在读取人脸预览'}</p>
      )}
    </div>
  );
}
