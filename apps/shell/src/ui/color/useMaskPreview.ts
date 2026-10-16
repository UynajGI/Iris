import { useEffect, useMemo, useRef, useState } from 'react';
import type { IrisClient } from '../../client.js';
import { EditingClient, type EditDocument, type EditRecord } from '../../editing-client.js';
import { EditingPreviewQueue } from '../../editing-preview-queue.js';
import { maskGridKey } from '../../editing-mask-preview.js';

type Input = {
  record: EditRecord;
  document: EditDocument;
  key: string;
  grid: string;
  sequence: number;
};

export function useMaskPreview(
  client: IrisClient,
  record: EditRecord,
  document: EditDocument,
  maskId: string
) {
  const api = useMemo(() => new EditingClient(client), [client]);
  const session = useMemo(() => crypto.randomUUID(), [record.id, maskId]);
  const sequence = useRef(0);
  const queue = useRef<EditingPreviewQueue<Input, Blob> | null>(null);
  const [attempt, setAttempt] = useState(0);
  const [result, setResult] = useState<{
    key: string;
    grid: string;
    scope: string;
    url: string;
  } | null>(null);
  const [error, setError] = useState<{ key: string; message: string } | null>(null);
  const grid = maskGridKey(document);
  const scope = JSON.stringify([record.id, record.revision, maskId]);
  const key = JSON.stringify([scope, document, attempt]);
  const latest = useRef({ key, grid, scope });
  latest.current = { key, grid, scope };
  useEffect(() => {
    const pending = new EditingPreviewQueue<Input, Blob>({
      render: (input, signal) =>
        api.maskPreview(input.record, input.document, maskId, session, input.sequence, signal),
      result: (blob, input) => {
        const inputScope = JSON.stringify([input.record.id, input.record.revision, maskId]);
        if (latest.current.scope !== inputScope || latest.current.grid !== input.grid) return;
        setResult({
          key: input.key,
          grid: input.grid,
          scope: inputScope,
          url: URL.createObjectURL(blob),
        });
      },
      error: (reason, input) => {
        if (latest.current.key !== input.key) return;
        const message =
          reason instanceof Error && reason.message.includes('coordinates differ')
            ? '此蒙版与照片的裁切坐标不同，无法显示覆盖'
            : '蒙版覆盖读取失败';
        setError({ key: input.key, message });
      },
      busy: () => {},
      cancel: (input) => {
        void api.cancel(session, input.sequence).catch(() => {});
      },
    });
    queue.current = pending;
    return () => {
      pending.dispose();
      if (queue.current === pending) queue.current = null;
      void api.close(session).catch(() => {});
    };
  }, [api, session, maskId]);
  useEffect(() => {
    queue.current?.push({ record, document, key, grid, sequence: ++sequence.current });
  }, [key, session]);
  useEffect(
    () => () => {
      if (result) URL.revokeObjectURL(result.url);
    },
    [result]
  );
  const failure = error?.key === key ? error.message : null;
  const current = result?.scope === scope && result.grid === grid && !failure ? result : null;
  return {
    result: current,
    pending: current?.key !== key,
    error: failure,
    retry: () => setAttempt((value) => value + 1),
  };
}
