import { readPhotoAnalysis } from '../analysis-readout.js';
import type { Action, Photo } from '../types.js';
import { Icon, type IconName } from './Icon.js';
import type { FaceBox } from './ZoomViewer.js';

export const decisionNames: Record<Action, string> = {
  pending: '未标记',
  keep: '保留',
  flag: '已标记',
  reject: '标记移除',
};
export const decisionIcons: Record<Exclude<Action, 'pending'>, IconName> = {
  keep: 'done',
  flag: 'flag',
  reject: 'close',
};
export const verdictNames = {
  recommend: '建议保留',
  review: '需要复核',
  reject_suggest: '建议移除',
};
export const componentNames: Record<string, string> = {
  eyes: '眼部状态',
  sharpness: '清晰度',
  face: '面部质量',
  exposure: '曝光与构图',
  smile: '表情',
};
export const tallyOrder = ['keep', 'flag', 'reject', 'pending'] as const;

export function Stars({ rating }: { rating: number }) {
  return (
    <span className="stars" role="img" aria-label={`${rating} 星`}>
      {Array.from({ length: 5 }, (_, index) => (
        <Icon key={index} name="star" className={index < rating ? 'on' : 'off'} />
      ))}
    </span>
  );
}
export function DecisionBadge({ decision }: { decision: Action }) {
  if (decision === 'pending') return null;
  return (
    <span className={`badge badge-${decision}`}>
      <Icon name={decisionIcons[decision]} />
      {decisionNames[decision]}
    </span>
  );
}
export function ScoreChip({ photo }: { photo: Photo }) {
  const analysis = readPhotoAnalysis(photo);
  return analysis.score === null ? null : (
    <span
      className={`score-chip verdict-${analysis.verdict ?? 'none'}`}
      title={analysis.verdict ? verdictNames[analysis.verdict] : undefined}
    >
      {Math.round(analysis.score)}
    </span>
  );
}
export function formatBytes(bytes: number) {
  return bytes >= 1048576
    ? `${(bytes / 1048576).toFixed(1)} MB`
    : `${Math.max(1, Math.round(bytes / 1024))} KB`;
}
export function formatTaken(value: string | null | undefined) {
  if (!value) return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? value
    : date.toLocaleString('zh-CN', {
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
      });
}

/** Face rectangles from current analysis, normalized against the analyzed raster. */
export function faceBoxes(photo: Photo | undefined): FaceBox[] {
  const current = photo ? readPhotoAnalysis(photo).current : null;
  if (!current || !current.width || !current.height) return [];
  return current.faces
    .filter((face) => face.bbox.length === 4 && face.bbox.every(Number.isFinite))
    .map((face) => {
      const [x, y, w, h] = face.bbox as [number, number, number, number];
      const eyes = [face.left_eye?.state, face.right_eye?.state];
      const closed = eyes.includes('closed'),
        uncertain = eyes.includes('uncertain') || eyes.includes(undefined);
      return {
        x: x / current.width,
        y: y / current.height,
        w: w / current.width,
        h: h / current.height,
        label: closed ? '闭眼' : uncertain ? '眼部不确定' : '睁眼',
        tone: closed ? 'bad' : uncertain ? 'warn' : 'good',
      };
    });
}
