import { useEffect, useState } from 'react';
import { nativeDetailBlob } from '../native-detail-image.js';
import type { DetailEvidence } from '../types.js';

export function NativeDetailImage({ evidence, zoom }: { evidence: DetailEvidence; zoom: boolean }) {
  const [url, setUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let local: string | undefined;
    setFailed(false);
    try {
      local = URL.createObjectURL(nativeDetailBlob(evidence.image_data_url!));
      setUrl(local);
    } catch {
      setFailed(true);
      setUrl(null);
    }
    return () => {
      if (local) URL.revokeObjectURL(local);
    };
  }, [evidence.image_data_url]);
  if (failed) return <p className="inline-error">区域图像无法显示，请重新复核。</p>;
  return (
    <div
      className="native-detail-crop"
      data-zoom={zoom}
      tabIndex={0}
      aria-label="原图复核区域，可滚动查看"
    >
      {url && (
        <img
          src={url}
          alt="从原图读取的复核区域"
          width={evidence.display_width}
          height={evidence.display_height}
          onError={() => setFailed(true)}
        />
      )}
    </div>
  );
}
