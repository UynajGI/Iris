import type { AnalysisStatus, Photo, ScoreBreakdown, VisionAnalysis } from './types.js';

export interface PhotoAnalysisReadout {
  status: AnalysisStatus | 'unknown';
  /** Raw server record remains accessible for historical inspection. */
  analysis: VisionAnalysis | null;
  /** Only the server can declare compatibility with its scanned source/settings. */
  current: VisionAnalysis | null;
  score: number | null;
  verdict: VisionAnalysis['verdict'] | null;
  scoreBreakdown: ScoreBreakdown | null;
}

/** No version strings, scoring formulas, or accuracy claims are inferred by the client. */
export function readPhotoAnalysis(photo: Photo): PhotoAnalysisReadout {
  const analysis = photo.analysis ?? null;
  const declared: unknown = photo.analysis_status;
  const status = !analysis ? 'missing' : declared === 'current' || declared === 'stale' ? declared : 'unknown';
  const current = status === 'current' ? analysis : null;
  return { status, analysis, current, score: current?.composite_score ?? null, verdict: current?.verdict ?? null, scoreBreakdown: current?.score_breakdown ?? null };
}
