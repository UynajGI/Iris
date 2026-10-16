import type { CSSProperties, ReactNode } from 'react';
import { readPhotoAnalysis } from '../analysis-readout.js';
import type { Photo, VisionAnalysis } from '../types.js';
import { Icon } from './Icon.js';
import { componentNames, formatBytes, verdictNames } from './photo-readouts.js';
import { previewSource, selectionReason } from '../selection-readout.js';
import type { IrisClient } from '../client.js';
import { CaptureMetadata } from './CaptureMetadata.js';

export function AnalysisPanel({ photo }: { photo: Photo }) {
  const analysis = readPhotoAnalysis(photo);
  const verdict = analysis.verdict
    ? verdictNames[analysis.verdict]
    : analysis.status === 'stale'
      ? '需要重新分析'
      : '尚未分析';
  if (!analysis.current)
    return (
      <div className="analysis-empty">
        <Icon name="auto-awesome" />
        <div>
          <p>{verdict}</p>
          <p className="muted">运行“智能分析”后，这里会显示人脸、眼部与清晰度评估。</p>
        </div>
      </div>
    );
  const faces = analysis.current.faces;
  const closed = faces.filter(
    (face) => face.left_eye?.state === 'closed' || face.right_eye?.state === 'closed'
  ).length;
  const components =
    analysis.scoreBreakdown?.components.filter(
      (item) => item.score !== null && item.score !== undefined
    ) ?? [];
  const score = analysis.score === null ? null : Math.round(analysis.score);
  return (
    <div className="analysis">
      <div className="analysis-head">
        <div
          className={`score-ring verdict-${analysis.verdict ?? 'none'}`}
          style={{ '--p': score ?? 0 } as CSSProperties}
        >
          <svg viewBox="0 0 36 36" aria-hidden="true">
            <circle className="ring-track" cx="18" cy="18" r="15.5" />
            <circle className="ring-value" cx="18" cy="18" r="15.5" pathLength="100" />
          </svg>
          <span className="ring-score tabular">{score ?? '—'}</span>
        </div>
        <div className="analysis-summary">
          <p className={`analysis-verdict verdict-${analysis.verdict ?? 'none'}`}>{verdict}</p>
          <p className="muted analysis-faces">
            <Icon name="face" />
            {faces.length
              ? `${faces.length} 张人脸${closed ? ` · ${closed} 人闭眼` : ''}`
              : '未检测到人脸'}
          </p>
        </div>
      </div>
      <p className="muted analysis-basis">{previewSource(photo)} · 技术评估不代表审美</p>
      <SelectionReasons evidence={analysis.current.selection_evidence} />
      {components.length > 0 && (
        <ul className="meters">
          {components.map((item) => {
            const value = Math.max(0, Math.min(100, item.score ?? 0));
            return (
              <li key={item.id}>
                <span className="meter-label">{componentNames[item.id] ?? item.id}</span>
                <span className="meter" aria-hidden="true">
                  <span
                    style={{ width: `${value}%` }}
                    className={value >= 70 ? 'good' : value >= 45 ? 'warn' : 'bad'}
                  />
                </span>
                <span className="tabular meter-value">{Math.round(value)}</span>
              </li>
            );
          })}
        </ul>
      )}
      {analysis.current.warnings.length > 0 && (
        <details className="analysis-warnings">
          <summary>{analysis.current.warnings.length} 条分析提示</summary>
          {analysis.current.warnings.map((message, index) => (
            <p key={index} className="muted">
              {message}
            </p>
          ))}
        </details>
      )}
    </div>
  );
}

function SelectionReasons({ evidence }: { evidence: VisionAnalysis['selection_evidence'] }) {
  if (!evidence) return null;
  return (
    <ul className="selection-reasons">
      {evidence.reasons
        .filter((item) => item.code !== 'preview_measurement')
        .map((item) => (
          <li key={item.code} data-kind={item.kind}>
            {selectionReason(item.code)}
          </li>
        ))}
    </ul>
  );
}

export function Metadata({ photo, client }: { photo: Photo; client: IrisClient }) {
  const rows: [string, ReactNode][] = [
    ['文件名', <span className="filename">{photo.filename}</span>],
    [
      '尺寸',
      <span className="tabular">
        {photo.width} × {photo.height}
      </span>,
    ],
    ['像素', <span className="tabular">{((photo.width * photo.height) / 1e6).toFixed(1)} MP</span>],
    ['格式', photo.format.toUpperCase()],
    ['大小', <span className="tabular">{formatBytes(photo.size_bytes)}</span>],
  ];
  return (
    <div className="metadata">
      <dl className="meta-table">
        {rows.map(([label, value]) => (
          <div key={label} className="meta-row">
            <dt>{label}</dt>
            <dd>{value}</dd>
          </div>
        ))}
      </dl>
      <details>
        <summary>文件位置</summary>
        <p className="filename path-text">{photo.path}</p>
      </details>
      <CaptureMetadata photo={photo} client={client} />
    </div>
  );
}
