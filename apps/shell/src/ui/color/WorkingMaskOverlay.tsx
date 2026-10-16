import { useState } from 'react';
import type { IrisClient } from '../../client.js';
import type { EditDocument, EditRecord } from '../../editing-client.js';
import { maskGridKey } from '../../editing-mask-preview.js';
import { useMaskPreview } from './useMaskPreview.js';

export type MaskPreviewSource = {
  record: EditRecord;
  document: EditDocument;
  displayed: EditDocument | null;
};

export function WorkingMaskOverlay({
  client,
  source,
  maskId,
}: {
  client: IrisClient;
  source: MaskPreviewSource;
  maskId: string;
}) {
  const overlay = useMaskPreview(client, source.record, source.document, maskId);
  const [loaded, setLoaded] = useState<string | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const result = overlay.result;
  const url = result?.url;
  const aligned = !!source.displayed && result?.grid === maskGridKey(source.displayed);
  const error = overlay.error ?? (url && failed === url ? '蒙版覆盖无法显示' : null);
  return (
    <>
      {url && (
        <img
          key={url}
          src={url}
          alt="红色范围蒙版覆盖"
          draggable={false}
          className="grade-mask-overlay"
          style={{ visibility: aligned && loaded === url && !error ? 'visible' : 'hidden' }}
          onLoad={() => setLoaded(url)}
          onError={() => setFailed(url)}
        />
      )}
      {(error || overlay.pending || loaded !== url || !aligned) && (
        <div
          className="grade-mask-preview-status"
          role="status"
          onPointerDown={(event) => event.stopPropagation()}
        >
          {error ?? '正在更新蒙版覆盖…'}
          {error && (
            <button type="button" onClick={overlay.retry}>
              重试
            </button>
          )}
        </div>
      )}
    </>
  );
}
