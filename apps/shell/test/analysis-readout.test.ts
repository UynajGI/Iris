import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readPhotoAnalysis, type Photo, type VisionAnalysis } from '../src/index.js';

const analysis: VisionAnalysis = {
  version: 'server-selected-engine', width: 640, height: 480, original_width: 640,
  original_height: 480, orientation: 1, sharpness_lap: 110, sharpness_fft: 9,
  exposure: { mean: 127, highlight_clip: 0, shadow_clip: 0, verdict: 'normal' },
  phash: '0', structure: [], warnings: [], composite_score: 73.125, verdict: 'review',
  faces: [{ index: 0, bbox: [0, 0, 100, 100], confidence: 0.95, landmarks: [],
    quality: { method: 'server-crop-observation', crop_width: 100, crop_height: 100,
      sharpness_lap: 80, exposure: { mean: 120, highlight_clip: 0, shadow_clip: 0, verdict: 'normal' },
      sharpness_score: 70, exposure_score: 90, resolution_score: 60, score: 74 } }],
  score_breakdown: { method: 'server-mapping', effective_weight_total: 0.15, components: [
    { id: 'face', score: 74, configured_weight: 0.15, effective_weight: 1, contribution: 74,
      observed_count: 1, total_count: 1, missing_reason: null,
      terms: [{ id: 'crop', raw: [{ name: 'sharpness_lap', value: 80 }], score: 74,
        weight: 1, contribution: 74, mapping: 'server-provided-formula', missing_reason: null }] },
  ] },
};
const photo: Photo = {
  id: 1, project_id: 1, filename: 'test.jpg', path: 'test.jpg', format: 'jpeg',
  mtime: 1, size_bytes: 100, width: 640, height: 480, missing: false, quarantined: false,
  decision: 'pending', analysis_status: 'current', analysis,
};

test('current readout preserves server score, observations and formulas without recomputation', () => {
  const readout = readPhotoAnalysis(photo);
  assert.equal(readout.status, 'current');
  assert.equal(readout.current, analysis);
  assert.equal(readout.score, 73.125);
  assert.equal(readout.verdict, 'review');
  assert.equal(readout.scoreBreakdown, analysis.score_breakdown);
  assert.equal(readout.current?.faces[0]?.quality, analysis.faces[0]?.quality);
});

test('stale readout keeps history but withholds current scores and suggestions', () => {
  const readout = readPhotoAnalysis({ ...photo, analysis_status: 'stale' });
  assert.equal(readout.status, 'stale');
  assert.equal(readout.analysis, analysis);
  assert.equal(readout.current, null);
  assert.equal(readout.score, null);
  assert.equal(readout.verdict, null);
  assert.equal(readout.scoreBreakdown, null);
});

test('older payload without authoritative freshness cannot become current', () => {
  const { analysis_status: _status, ...legacy } = photo;
  const readout = readPhotoAnalysis(legacy as Photo);
  assert.equal(readout.status, 'unknown');
  assert.equal(readout.analysis, analysis);
  assert.equal(readout.current, null);
  assert.equal(readout.score, null);
  assert.equal(readout.verdict, null);
  assert.equal(readout.scoreBreakdown, null);
});

test('missing analysis and unobserved face quality remain absent, never fabricated as zero', () => {
  assert.equal(readPhotoAnalysis({ ...photo, analysis: null }).status, 'missing');
  assert.equal(readPhotoAnalysis({ ...photo, analysis: null }).score, null);
  const { score_breakdown: _breakdown, ...withoutBreakdown } = analysis;
  const { quality: _quality, ...face } = analysis.faces[0]!;
  const readout = readPhotoAnalysis({ ...photo, analysis: { ...withoutBreakdown, faces: [face] } });
  assert.equal(readout.current?.faces[0]?.quality, undefined);
  assert.equal(readout.scoreBreakdown, null);
});
