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
  const [failed, setFailed] = useState<string | null>(null);
  const result = overlay.result;
  const url = result?.url;
  const aligned = !!source.displayed && result?.grid === maskGridKey(source.displayed);
  const error = overlay.error ?? (url && failed === url ? '蒙版覆盖无法显示' : null);
  return (
    <>
      {url && (
        <img
          src={url}
          alt="红色范围蒙版覆盖"
          draggable={false}
          className="grade-mask-overlay"
          style={{ visibility: aligned && !error ? 'visible' : 'hidden' }}
          onError={() => setFailed(url)}
        />
      )}
      {error && (
        <div
          className="grade-mask-preview-status"
          role="status"
          onPointerDown={(event) => event.stopPropagation()}
        >
          {error}
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
