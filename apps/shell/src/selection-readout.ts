import { readPhotoAnalysis } from './analysis-readout.js';
import type { Photo } from './types.js';

const reasons: Record<string, string> = {
  preview_measurement: '依据缩小预览，细节请放大确认',
  no_face_detected: '未检测到人脸，按画面技术质量评估',
  closed_eye_review: '有闭眼迹象，请结合动作与拍摄意图复核',
  eye_evidence_limited: '部分眼睛无法可靠判断',
  face_detail_limited: '部分人脸预览细节不足，请查看原图',
  tonal_style_review: '画面偏亮或偏暗，亮暗风格本身不扣分',
  preview_shadow_clip: '预览中有较大面积暗部裁切',
  preview_highlight_clip: '预览中有较大面积高光裁切',
  raw_recovery_unknown: 'RAW 可恢复范围未知，预览不能证明不可挽救',
  low_preview_detail: '预览细节较少，请确认主体是否虚焦',
  invalid_measurements: '部分技术指标不可用',
  omitted_faces: '部分人脸未完成分析',
  analysis_unavailable: '分析缺失或已过期，暂不比较',
  review_observations: '有待复核项，请查看判断依据',
  technical_reference: '组内技术参考',
  close_quality: '技术质量接近，可同时保留待选',
  face_detail_lower: '面部质量指标低于参考照片',
  face_detail_higher: '面部质量指标高于参考照片',
  preview_detail_lower: '预览清晰度指标低于参考照片',
  preview_detail_higher: '预览清晰度指标高于参考照片',
  different_face_count: '检测到的人脸数量不同，请确认人物是否完整',
};
export const selectionReason = (code: string) => reasons[code] ?? '请结合原图复核';

export function previewSource(photo: Photo): string {
  const source = readPhotoAnalysis(photo).current?.preview_source;
  if (source === 'raw_embedded_jpeg') return '相机内嵌预览';
  if (source === 'raw_developed') return 'RAW 解码预览';
  if (source === 'heic_thumbnail') return 'HEIC 缩略预览';
  return '缩小预览';
}

export interface FaceRegion {
  index: number;
  x: number;
  y: number;
  w: number;
  h: number;
  width: number;
  height: number;
}

/** Boxes are already in EXIF-oriented preview coordinates. No identity match is inferred. */
export function faceRegions(photo: Photo): FaceRegion[] {
  const analysis = readPhotoAnalysis(photo).current;
  if (!analysis || analysis.width <= 0 || analysis.height <= 0) return [];
  const { width, height } = analysis;
  return analysis.faces
    .flatMap((face, index) => {
      const [x, y, w, h] = face.bbox;
      if (![x, y, w, h].every((n) => Number.isFinite(n)) || w! <= 0 || h! <= 0) return [];
      const left = Math.max(0, x!),
        top = Math.max(0, y!);
      const right = Math.min(width, x! + w!),
        bottom = Math.min(height, y! + h!);
      if (right <= left || bottom <= top) return [];
      const edge = Math.min(Math.max(right - left, bottom - top) * 1.6, width, height);
      const cx = (left + right) / 2,
        cy = (top + bottom) / 2;
      return [
        {
          index,
          x: Math.max(0, Math.min(width - edge, cx - edge / 2)),
          y: Math.max(0, Math.min(height - edge, cy - edge / 2)),
          w: edge,
          h: edge,
          width,
          height,
        },
      ];
    })
    .sort((a, b) => a.x - b.x || a.y - b.y);
}

export function originalDetailAvailable(photo: Photo): boolean {
  return (
    ['jpeg', 'jpg', 'png', 'webp'].includes(photo.format.toLowerCase()) &&
    photo.width > 0 &&
    photo.height > 0 &&
    photo.width * photo.height <= 60_000_000 &&
    photo.size_bytes <= 64 * 1024 * 1024
  );
}

export function originalDetailPhotoId(photo: Photo, active: boolean): number | null {
  return active && originalDetailAvailable(photo) ? photo.id : null;
}

/** Map oriented analysis coordinates onto the decoded original, without downscaling it. */
export function scaleFaceRegion(region: FaceRegion, width: number, height: number): FaceRegion {
  const sx = width / region.width,
    sy = height / region.height;
  return {
    ...region,
    x: region.x * sx,
    y: region.y * sy,
    w: region.w * sx,
    h: region.h * sy,
    width,
    height,
  };
}
