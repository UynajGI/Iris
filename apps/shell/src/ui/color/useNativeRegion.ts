import { useEffect, useMemo, useRef, useState } from 'react';
import type { IrisClient } from '../../client.js';
import { EditingClient, type EditDocument, type EditRecord } from '../../editing-client.js';
import type { NativeRegion, RegionView } from '../../editing-region.js';

export function useNativeRegion(
  client: IrisClient,
  record: EditRecord,
  document: EditDocument,
  view: RegionView | null
) {
  const api = useMemo(() => new EditingClient(client), [client]);
  const session = useMemo(() => crypto.randomUUID(), [record.id]);
  const sequence = useRef(0);
  const [attempt, setAttempt] = useState(0);
  const [result, setResult] = useState<{ key: string; region: NativeRegion; url: string } | null>(
    null
  );
  const [failure, setFailure] = useState<{ key: string; message: string } | null>(null);
  const key = JSON.stringify([record.id, record.revision, document, view, attempt]);
  const latestKey = useRef(key);
  latestKey.current = key;
  useEffect(() => {
    if (!view) return;
    const controller = new AbortController();
    const number = ++sequence.current;
    let active = true;
    let started = false;
    const timer = setTimeout(() => {
      started = true;
      void api
        .nativeRegion(record, document, view, session, number, controller.signal)
        .then((region) => {
          if (!active || latestKey.current !== key) return;
          setResult({ key, region, url: URL.createObjectURL(region.blob) });
        })
        .catch(() => {
          if (active && latestKey.current === key) setFailure({ key, message: '原尺寸读取失败' });
        });
    }, 350);
    return () => {
      active = false;
      clearTimeout(timer);
      controller.abort();
      if (started) void api.cancel(session, number).catch(() => {});
    };
  }, [api, session, key]);
  useEffect(
    () => () => {
      if (result) URL.revokeObjectURL(result.url);
    },
    [result]
  );
  useEffect(
    () => () => {
      void api.close(session).catch(() => {});
    },
    [api, session]
  );
  return {
    result: result?.key === key ? result : null,
    error: failure?.key === key ? failure.message : null,
    retry: () => setAttempt((value) => value + 1),
  };
}
