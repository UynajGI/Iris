import { Select } from '../Select.js';
import { useEffect, useMemo, useState } from 'react';
import type { IrisClient } from '../../client.js';
import type { Photo } from '../../types.js';
import {
  EditingClient,
  type EditRecord,
  type EditDocument,
  type RenderedExportStatus,
} from '../../editing-client.js';
import { selectProjectFolder } from '../../native-dialog.js';
import { preferredEdit } from '../../editing-preferences.js';
import { ExportAdvanced } from './ExportAdvanced.js';
import { Button } from '../components.js';
import { Icon } from '../Icon.js';

const stateNames: Record<string, string> = {
  queued: '等待导出',
  running: '正在导出',
  completed: '导出完成',
  failed: '导出失败',
  cancelled: '已取消',
  interrupted: '导出中断',
  publishing: '正在完成',
};
export function RenderedExportPanel({
  client,
  photo,
}: {
  client: IrisClient;
  photo: Photo | undefined;
}) {
  const api = useMemo(() => new EditingClient(client), [client]);
  const [versions, setVersions] = useState<EditRecord[]>([]);
  const [version, setVersion] = useState('');
  const [custom, setCustom] = useState<{ id: string; document: EditDocument } | null>(null);
  const [format, setFormat] = useState<'jpeg8' | 'png16' | 'tiff16'>('jpeg8');
  const [quality, setQuality] = useState(92);
  const [destination, setDestination] = useState('');
  const [filename, setFilename] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [jobs, setJobs] = useState<RenderedExportStatus[]>([]);
  const extension = format === 'jpeg8' ? 'jpg' : format === 'png16' ? 'png' : 'tif';
  const selectedVersion = versions.find(
    (item) => item.id === version && item.photo_id === photo?.id
  );
  useEffect(() => {
    const controller = new AbortController();
    let active = true;
    void api
      .recentExports(controller.signal)
      .then((items) => {
        if (active) setJobs(items);
      })
      .catch(() => {});
    return () => {
      active = false;
      controller.abort();
    };
  }, [api]);
  useEffect(() => {
    let active = true;
    setVersions([]);
    setVersion('');
    setCustom(null);
    setError(null);
    setFilename(photo ? `${photo.filename.replace(/\.[^.]+$/, '')}-edited` : '');
    if (photo)
      void api
        .list(photo.id)
        .then((items) => {
          if (!active) return;
          setVersions(items);
          setVersion(preferredEdit(photo.id, items)?.id ?? '');
        })
        .catch((reason) => {
          if (active) setError(reason instanceof Error ? reason.message : String(reason));
        });
    return () => {
      active = false;
    };
  }, [api, photo?.id]);
  const activeJobs = jobs
    .filter((job) => ['queued', 'running', 'publishing'].includes(job.state))
    .map((job) => job.id)
    .join(',');
  useEffect(() => {
    if (!activeJobs) return;
    const controller = new AbortController();
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const updates = await Promise.all(
          activeJobs.split(',').map((id) => api.exportStatus(id, controller.signal))
        );
        if (active)
          setJobs((items) =>
            items.map((job) => updates.find((update) => update.id === job.id) ?? job)
          );
      } catch (reason) {
        if (active)
          setError(
            `导出状态暂不可用：${reason instanceof Error ? reason.message : String(reason)}`
          );
      }
      if (active) timer = setTimeout(() => void poll(), 1000);
    };
    void poll();
    return () => {
      active = false;
      clearTimeout(timer);
      controller.abort();
    };
  }, [api, activeJobs]);
  const run = async (action: () => Promise<void>) => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  };
  const queue = async () => {
    const record = selectedVersion;
    if (!record) return;
    const result = await api.exports([
      {
        document_id: record.id,
        expected_revision: record.revision,
        destination_directory: destination,
        filename: `${filename}.${extension}`,
        format: format === 'jpeg8' ? { format, quality } : { format },
        ...(custom?.id === record.id ? { document: custom.document } : {}),
      },
    ]);
    setJobs((items) => [...result, ...items]);
  };
  return (
    <section className="rendered-export" aria-label="调色成片导出">
      <h2>成片导出</h2>
      {photo ? (
        <>
          <div className="rendered-export-source">
            <Icon name="image" />
            <span className="filename">{photo.filename}</span>
          </div>
          <div className="rendered-export-fields">
            <label>
              调色版本
              <Select
                value={version}
                aria-label="调色版本"
                disabled={busy || !versions.length}
                onValueChange={(nextValue) => {
                  setVersion(nextValue);
                  setCustom(null);
                }}
              >
                {versions.length ? (
                  versions.map((item) => (
                    <option key={item.id} value={item.id}>
                      {item.name}
                    </option>
                  ))
                ) : (
                  <option value="">尚无已保存版本</option>
                )}
              </Select>
            </label>
            <label>
              文件格式
              <Select
                value={format}
                aria-label="文件格式"
                disabled={busy}
                onValueChange={(nextValue) => setFormat(nextValue as typeof format)}
              >
                <option value="jpeg8">JPEG · 8 位</option>
                <option value="png16">PNG · 16 位</option>
                <option value="tiff16">TIFF · 16 位</option>
              </Select>
            </label>
            {format === 'jpeg8' && (
              <label>
                JPEG 质量
                <input
                  type="number"
                  value={quality}
                  aria-label="JPEG 质量"
                  min={1}
                  max={100}
                  disabled={busy}
                  onChange={(event) =>
                    setQuality(Math.max(1, Math.min(100, Number(event.target.value))))
                  }
                />
              </label>
            )}
            <label>
              文件名
              <div className="rendered-export-name">
                <input
                  value={filename}
                  aria-label="文件名"
                  disabled={busy}
                  onChange={(event) => setFilename(event.target.value)}
                />
                <span>.{extension}</span>
              </div>
            </label>
            <label>
              保存位置
              <div className="rendered-export-destination">
                <input
                  value={destination}
                  aria-label="保存位置"
                  disabled={busy}
                  placeholder="选择导出文件夹"
                  onChange={(event) => setDestination(event.target.value)}
                />
                <Button
                  disabled={busy}
                  onClick={() =>
                    void run(async () => {
                      if (!window.__TAURI__) throw new Error('请在桌面应用中选择文件夹');
                      const folder = await selectProjectFolder(window.__TAURI__.core);
                      if (folder) setDestination(folder);
                    })
                  }
                >
                  选择
                </Button>
              </div>
            </label>
          </div>
          {selectedVersion && (
            <ExportAdvanced
              key={selectedVersion.id}
              client={client}
              record={selectedVersion}
              document={
                custom?.id === selectedVersion.id ? custom.document : selectedVersion.document
              }
              disabled={busy}
              onChange={(document) => setCustom({ id: selectedVersion.id, document })}
            />
          )}
          {error && (
            <p className="inline-error" role="alert">
              {error}
            </p>
          )}
          <Button
            primary
            disabled={
              busy || !version || !destination.trim() || !filename.trim() || /[\\/]/.test(filename)
            }
            onClick={() => void run(queue)}
          >
            <Icon name="export" />
            {busy ? '正在提交' : '导出成片'}
          </Button>
        </>
      ) : (
        <p className="muted">选择一张照片</p>
      )}
      {jobs.length > 0 && (
        <ul className="rendered-export-jobs">
          {jobs.map((job) => (
            <li key={job.id}>
              <div>
                <strong>{stateNames[job.state] ?? job.state}</strong>
                <span className="filename">{job.destination}</span>
                {job.error && <span className="inline-error">{job.error}</span>}
              </div>
              {['queued', 'running'].includes(job.state) && (
                <Button
                  disabled={busy}
                  onClick={() =>
                    void run(async () => {
                      const result = await api.exportCommand(job.id, 'cancel');
                      setJobs((items) =>
                        items.map((item) => (item.id === result.id ? result : item))
                      );
                    })
                  }
                >
                  取消
                </Button>
              )}
              {['failed', 'cancelled', 'interrupted'].includes(job.state) && (
                <Button
                  disabled={busy}
                  onClick={() =>
                    void run(async () => {
                      const result = await api.exportCommand(
                        job.id,
                        job.state === 'interrupted' ? 'recover' : 'retry'
                      );
                      setJobs((items) =>
                        items.map((item) => (item.id === result.id ? result : item))
                      );
                    })
                  }
                >
                  重试
                </Button>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
