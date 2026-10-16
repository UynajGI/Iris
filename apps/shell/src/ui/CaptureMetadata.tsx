import { useEffect, useState } from 'react';
import type { IrisClient } from '../client.js';
import type { Photo, PhotoMetadata } from '../types.js';
import {
  captureDate,
  cameraName,
  decimal,
  ev,
  exposurePrograms,
  flashLabel,
  meteringModes,
  shutter,
  tagLabel,
} from '../metadata-format.js';
import { Button } from './components.js';

function Rows({ rows }: { rows: [string, string][] }) {
  return (
    <dl className="meta-table">
      {rows.map(([label, value]) => (
        <div key={label} className="meta-row">
          <dt>{label}</dt>
          <dd>{value}</dd>
        </div>
      ))}
    </dl>
  );
}

export function CaptureMetadata({ photo, client }: { photo: Photo; client: IrisClient }) {
  const identity = `${photo.id}:${photo.mtime}:${photo.size_bytes}`;
  const [attempt, setAttempt] = useState(0);
  const [result, setResult] = useState<{
    identity: string;
    data?: PhotoMetadata;
    error?: string;
  } | null>(null);
  useEffect(() => {
    const controller = new AbortController();
    setResult(null);
    void client.photoMetadata(photo.id, controller.signal).then(
      (data) => {
        if (!controller.signal.aborted) setResult({ identity, data });
      },
      (error: unknown) => {
        if (!controller.signal.aborted)
          setResult({ identity, error: error instanceof Error ? error.message : String(error) });
      }
    );
    return () => controller.abort();
  }, [client, identity, photo.id, attempt]);
  const current = result?.identity === identity ? result : null;
  const data = current?.data;
  return (
    <section className="capture-metadata" aria-label="拍摄信息" aria-busy={!current}>
      <h3>拍摄信息</h3>
      {!current && (
        <p className="muted" role="status">
          正在读取
        </p>
      )}
      {current?.error && (
        <div role="alert">
          <p>拍摄信息读取失败</p>
          <p className="muted">{current.error}</p>
          <Button className="quiet" onClick={() => setAttempt((value) => value + 1)}>
            重新读取
          </Button>
        </div>
      )}
      {data && data.status !== 'available' && (
        <p className="muted">
          {data.status === 'not_found'
            ? '文件未包含 EXIF 拍摄信息'
            : data.status === 'read_limit'
              ? '拍摄信息超出快速读取范围'
              : '无法读取此文件的 EXIF 拍摄信息'}
        </p>
      )}
      {data?.status === 'available' && <CaptureFields data={data} />}
    </section>
  );
}

function CaptureFields({ data }: { data: PhotoMetadata }) {
  return (
    <>
      <Rows
        rows={[
          ['相机', cameraName(data.camera_make, data.camera_model)],
          ['镜头', data.lens_model ?? data.lens_make ?? '—'],
          ['光圈', data.f_number ? `f/${decimal(data.f_number)}` : '—'],
          ['快门', shutter(data.exposure_seconds)],
          ['ISO', decimal(data.iso, 0)],
          ['焦距', data.focal_mm ? `${decimal(data.focal_mm)} mm` : '—'],
          ['拍摄时间', captureDate(data.captured_at, data.captured_offset)],
        ]}
      />
      <details>
        <summary>更多拍摄信息</summary>
        <Rows
          rows={[
            ['等效焦距', data.focal_35mm ? `${data.focal_35mm} mm` : '—'],
            ['曝光补偿', ev(data.exposure_bias_ev)],
            ['曝光模式', tagLabel(data.exposure_program, exposurePrograms)],
            ['测光', tagLabel(data.metering_mode, meteringModes)],
            ['白平衡', tagLabel(data.white_balance, { 0: '自动', 1: '手动' })],
            ['闪光灯', flashLabel(data.flash)],
            ['镜头厂商', data.lens_make ?? '—'],
            ['拍摄者', data.artist ?? '—'],
            ['版权', data.copyright ?? '—'],
          ]}
        />
      </details>
    </>
  );
}
