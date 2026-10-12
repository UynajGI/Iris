import { useEffect, useState } from 'react';
import type { IrisClient } from '../client.js';
import type { DetailEvidence, Photo } from '../types.js';
import { faceRegions } from '../selection-readout.js';
import { mediaRevision } from '../media-cache.js';
import { Button } from './components.js';

/** Expensive original-region evidence is requested explicitly, never on every focus change. */
export function DetailReview({
  client,
  photo,
  faceIndex,
}: {
  client: IrisClient;
  photo: Photo;
  faceIndex?: number | null;
}) {
  const [selected, setSelected] = useState<number | null>(null);
  const index = faceIndex === undefined ? selected : faceIndex;
  return (
    <div className="detail-review">
      {faceIndex === undefined && (
        <label className="field">
          复核区域
          <select
            value={selected ?? ''}
            onChange={(event) =>
              setSelected(event.target.value === '' ? null : Number(event.target.value))
            }
          >
            <option value="">画面中央</option>
            {faceRegions(photo).map((face, i) => (
              <option key={face.index} value={face.index}>
                人脸 {i + 1}（从左到右）
              </option>
            ))}
          </select>
        </label>
      )}
      <DetailMeasurement
        key={`${photo.id}:${mediaRevision(photo)}:${index}`}
        client={client}
        photo={photo}
        faceIndex={index}
      />
    </div>
  );
}

function DetailMeasurement({
  client,
  photo,
  faceIndex,
}: {
  client: IrisClient;
  photo: Photo;
  faceIndex: number | null;
}) {
  const [result, setResult] = useState<DetailEvidence | null>(null);
  const [error, setError] = useState('');
  const [request, setRequest] = useState<AbortController | null>(null);
  useEffect(() => () => request?.abort(), [request]);
  const measure = async () => {
    const controller = new AbortController();
    setRequest(controller);
    setError('');
    setResult(null);
    try {
      const evidence = await client.detail(
        photo.id,
        { face_index: faceIndex, region: null },
        controller.signal
      );
      if (!controller.signal.aborted) setResult(evidence);
    } catch (cause) {
      if (!controller.signal.aborted) setError(String(cause));
    } finally {
      if (!controller.signal.aborted) setRequest(null);
    }
  };
  return (
    <>
      <Button className="ghost block" disabled={!!request} onClick={() => void measure()}>
        {request ? '正在复核细节…' : '复核原始细节'}
      </Button>
      {error && (
        <p className="inline-error" role="alert">
          细节复核未完成，可重试：{error}
        </p>
      )}
      {result && (
        <div className="detail-evidence" role="status">
          <strong>
            {result.state === 'full_detail'
              ? '原图区域测量'
              : result.state === 'preview_only'
                ? '仅有预览，原始细节待人工确认'
                : '此区域暂无法测量'}
          </strong>
          {result.state === 'full_detail' && (
            <>
              <p className="tabular">
                区域 {result.crop_width}×{result.crop_height} · 测量 {result.measured_width}×
                {result.measured_height}
              </p>
              <dl className="evidence-values">
                <dt>局部清晰度</dt>
                <dd>{result.sharpness_lap?.toFixed(1) ?? '未知'}</dd>
                <dt>高光裁切</dt>
                <dd>
                  {result.exposure
                    ? `${(result.exposure.highlight_clip * 100).toFixed(1)}%`
                    : '未知'}
                </dd>
              </dl>
              <p className="muted">
                数值随纹理、区域和测量尺寸变化，用于辅助复核；不自动改写评分或眼部判断。
              </p>
            </>
          )}
          {result.state !== 'full_detail' && (
            <p className="muted">格式、尺寸或可用分辨率受限，不代表照片有缺陷。</p>
          )}
        </div>
      )}
    </>
  );
}
