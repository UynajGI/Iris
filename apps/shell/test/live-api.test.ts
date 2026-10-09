import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, readdir, copyFile, mkdir, rm, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createInterface } from 'node:readline';
import { IrisClient, IrisStore, Transport, EventBus, readPhotoAnalysis, type Session, type VisionAnalysis } from '../src/index.js';

test('live daemon: JPG scan, media, decisions, undo, settings, export and events', {
  skip: !process.env.IRIS_TEST_DAEMON || !process.env.IRIS_TEST_PHOTOS,
  timeout: 90000,
}, async () => {
  const directory = await mkdtemp(join(tmpdir(), 'iris-l4-'));
  const photoRoot = join(directory, 'photos');
  const modelRoot = join(directory, 'models');
  await mkdir(photoRoot);
  await mkdir(modelRoot);
  const modelSource = process.env.IRIS_TEST_MODELS ?? fileURLToPath(new URL('../../../models/', import.meta.url));
  // Copy only the known default artifacts; never copy user-supplied optional weights.
  for (const name of ['manifest.json', 'yunet.onnx', 'face_landmarks_detector.onnx', 'face_blendshapes.onnx', 'onnxruntime.dll', 'niqe_params.json']) {
    await copyFile(join(modelSource, name), join(modelRoot, name));
  }
  const names = (await readdir(process.env.IRIS_TEST_PHOTOS!)).filter(name => /\.jpe?g$/i.test(name)).slice(0, 3);
  assert.ok(names.length > 0, 'live integration needs at least one JPG fixture');
  for (const name of names) await copyFile(join(process.env.IRIS_TEST_PHOTOS!, name), join(photoRoot, name));
  const child = spawn(process.env.IRIS_TEST_DAEMON!, ['--data-dir', join(directory, 'data'), '--model-dir', modelRoot, '--port', '0'], { stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true });
  const exit = new Promise<void>(resolve => child.once('exit', () => resolve()));
  let store: IrisStore | undefined; let bus: EventBus | undefined;
  try {
    const session = await new Promise<Session>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('daemon startup timeout')), 30000);
      const lines = createInterface({ input: child.stdout });
      lines.once('line', line => { clearTimeout(timer); lines.close(); try { resolve(JSON.parse(line) as Session); } catch { reject(new Error('invalid startup JSON')); } });
      child.once('error', error => { clearTimeout(timer); reject(error); });
      child.once('exit', () => { clearTimeout(timer); reject(new Error('daemon exited during startup')); });
    });
    const client = new IrisClient(new Transport(session));
    store = new IrisStore(client);
    await store.initialize(); await store.openProject(photoRoot);
    assert.equal(store.getSnapshot().bootstrap?.recovery_notice, null);
    const id = store.getSnapshot().project!.id;
    assert.equal(store.getSnapshot().detectorModels?.selected, 'yunet');
    assert.equal(store.getSnapshot().detectorModels?.detectors.find(model => model.provider === 'yunet')?.state, 'available');
    assert.equal(store.getSnapshot().detectorModels?.occlusion_selected, 'none');
    assert.equal(store.getSnapshot().detectorModels?.occlusion.find(model => model.provider === 'none')?.state, 'disabled');
    assert.equal((await client.project(id)).id, id);
    assert.ok((await client.projects()).some(project => project.id === id));
    const openedAt = (await client.project(id)).last_opened_at;
    await store.refresh(); await client.project(id);
    assert.equal((await client.project(id)).last_opened_at, openedAt, 'background reads do not reorder recent projects');
    await store.hideRecentProject(id);
    assert.equal(store.getSnapshot().project?.id, id);
    assert.ok(!(await client.projects()).some(project => project.id === id));
    await store.openRecentProject(id);
    assert.equal(store.getSnapshot().projects[0]?.id, id);
    assert.equal(store.getSnapshot().projects[0]?.hidden, false);
    assert.equal((await client.contract()).openapi, '3.1.0');
    let received = 0;
    bus = new EventBus(session, { onEvent: () => { received++; }, onState: () => {}, synchronize: async () => {} });
    bus.start();
    await store.run('scan');
    for (let attempt = 0; attempt < 300; attempt++) {
      const progress = await client.progress(id);
      if (progress.state === 'completed') break;
      assert.notEqual(progress.state, 'failed', JSON.stringify(progress.errors));
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    assert.equal((await client.progress(id)).state, 'completed');
    await store.refresh();
    assert.equal(store.getSnapshot().photos.length, names.length);
    const first = store.getSnapshot().photos[0]!;
    assert.ok((await client.media(first.id, 'thumb')).size > 0);
    assert.ok((await client.media(first.id, 'preview')).size > 0);
    assert.deepEqual(Buffer.from(await (await client.media(first.id, 'original')).arrayBuffer()), await readFile(join(photoRoot, first.filename)));
    assert.ok(Array.isArray(await client.groups(id)));
    await store.decide('keep');
    assert.equal((await client.photo(first.id)).decision, 'keep');
    await store.undo();
    assert.equal((await client.photo(first.id)).decision, 'pending');
    await store.decide('keep');
    const settings = await client.settings(id);
    await store.saveSettings({ ...settings, face_detector: 'scrfd_500m', scrfd_model_sha256: 'a'.repeat(64) });
    assert.equal(store.getSnapshot().detectorModels?.selected, 'scrfd_500m');
    assert.equal(store.getSnapshot().detectorModels?.detectors.find(model => model.provider === 'scrfd_500m')?.state, 'missing');
    await store.reanalyze();
    for (let attempt = 0; attempt < 300; attempt++) {
      if ((await client.progress(id)).state !== 'running') break;
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    const failed = await client.progress(id);
    assert.equal(failed.state, 'failed', 'missing selected SCRFD fails closed');
    assert.match(failed.errors.join(' '), /scrfd/i);
    assert.equal((await client.settings(id)).face_detector, 'scrfd_500m');
    assert.ok((await client.photos(id)).every(photo => photo.analysis === null), 'failed SCRFD analysis must not silently use YuNet');
    await store.saveSettings(settings);
    assert.equal(store.getSnapshot().detectorModels?.selected, 'yunet');
    assert.equal(settings.occlusion_provider, 'none');
    await assert.rejects(client.saveSettings(id, { ...settings, occlusion_provider: 'faceocc', occlusion_model_sha256: 'b'.repeat(64), occlusion_min_visible_fraction: null }), /400/);
    assert.equal((await client.settings(id)).occlusion_provider, 'none');
    // This explicit value is a test input, not a calibrated recommended threshold.
    await store.saveSettings({ ...settings, occlusion_provider: 'faceocc', occlusion_model_sha256: 'b'.repeat(64), occlusion_min_visible_fraction: 0.75 });
    assert.equal(store.getSnapshot().detectorModels?.occlusion_selected, 'faceocc');
    assert.equal(store.getSnapshot().detectorModels?.occlusion.find(model => model.provider === 'faceocc')?.state, 'missing');
    await store.reanalyze();
    for (let attempt = 0; attempt < 300; attempt++) {
      if ((await client.progress(id)).state !== 'running') break;
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    const occlusionFailure = await client.progress(id);
    assert.equal(occlusionFailure.state, 'failed', 'missing selected FaceOcc fails closed');
    assert.match(occlusionFailure.errors.join(' '), /faceocc/i);
    assert.equal((await client.settings(id)).occlusion_provider, 'faceocc');
    assert.ok((await client.photos(id)).every(photo => photo.analysis === null), 'FaceOcc failure must not silently analyze without occlusion gating');
    await store.saveSettings(settings);
    assert.equal(store.getSnapshot().detectorModels?.occlusion_selected, 'none');
    await client.decide(id, [first.id], 'pending', false);
    // Seed only this test's temporary DB to simulate a persisted pre-upgrade record.
    const priorVersion = 'iris-vision-v4-conservative-ear-fallback-2026-10-06';
    const legacy: VisionAnalysis = {
      version: priorVersion, width: 100, height: 100, original_width: 100, original_height: 100,
      orientation: 1, sharpness_lap: 100, sharpness_fft: 10,
      exposure: { mean: 127, highlight_clip: 0, shadow_clip: 0, verdict: 'normal' },
      faces: [], composite_score: 99, verdict: 'recommend', phash: '0', structure: [], warnings: [],
    };
    const { DatabaseSync } = await import('node:sqlite');
    const db = new DatabaseSync(join(directory, 'data', 'library.sqlite3'));
    try {
      db.prepare('INSERT OR REPLACE INTO analyses(photo_id,data,version,analyzed_at) VALUES(?,?,?,?)')
        .run(first.id, JSON.stringify({ ...legacy, settings }), priorVersion, new Date().toISOString());
    } finally { db.close(); }
    const stale = await client.photo(first.id);
    assert.equal(stale.analysis_status, 'stale');
    assert.equal(stale.analysis?.version, priorVersion);
    assert.equal(stale.analysis?.composite_score, 99, 'historical analysis remains inspectable');
    assert.equal(readPhotoAnalysis(stale).score, null);
    assert.equal(readPhotoAnalysis(stale).verdict, null);
    await store.refresh();
    assert.equal(store.getSnapshot().reanalysisRequired, true);
    assert.equal(store.photoAnalysis(first.id)?.status, 'stale');
    await assert.rejects(client.accept(id, { photo_ids: [first.id], category: 'recommend' }), /400|stale|analysis/i);
    assert.equal((await client.photo(first.id)).decision, 'pending');
    await store.reanalyze();
    for (let attempt = 0; attempt < 300; attempt++) {
      const progress = await client.progress(id);
      if (progress.state === 'completed') break;
      assert.notEqual(progress.state, 'failed', JSON.stringify(progress.errors));
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    assert.equal((await client.progress(id)).state, 'completed');
    await store.refresh();
    const current = await client.photo(first.id);
    assert.equal(current.analysis_status, 'current');
    assert.notEqual(current.analysis?.version, priorVersion);
    assert.ok(readPhotoAnalysis(current).scoreBreakdown);
    assert.equal(store.getSnapshot().reanalysisRequired, false);
    await client.decide(id, [first.id], 'keep', false);
    await store.refresh();
    await store.saveProfile({ name: 'integration', settings });
    await store.applyProfile('integration');
    await client.estimateProfile(id, 'integration');
    assert.equal((await client.exportCopy(id, join(directory, 'selected'))).written, 1);
    assert.equal((await client.exportXmp(id)).written, 1);
    assert.equal((await client.exportCsv(id, join(directory, 'decisions.csv'))).written, names.length);
    await store.decide('flag');
    await client.importCsv(id, join(directory, 'decisions.csv'));
    assert.equal((await client.photo(first.id)).decision, 'keep');
    await store.accept();
    const emptyAcceptance = await client.accept(id, { photo_ids: [], category: 'recommend' });
    assert.equal(emptyAcceptance.changed, 0);
    await store.accept({ photo_ids: [first.id], category: 'reject_suggest' });
    assert.equal((await client.photo(first.id)).decision, 'keep', 'scoped acceptance preserves manual decisions');
    assert.ok((await client.cache(id)).files > 0);
    await store.migrateCache(join(directory, 'cache-migrated'));
    assert.ok(store.getSnapshot().cache?.root.includes('cache-migrated'));
    const migration = store.getSnapshot().cacheMigrations.find(record => record.state === 'ready');
    assert.ok(migration, 'migration is recorded before old cache cleanup');
    await assert.rejects(store.cleanupPreviousCache(migration.id), /Review and confirm/);
    store.reviewCacheMigration(migration.id);
    await store.cleanupPreviousCache(migration.id);
    assert.equal(store.getSnapshot().cacheMigration?.state, 'cleaned');
    await client.cleanupCache(id);
    assert.equal((await client.cache(id)).files, 0);
    store.select([first.id]); await store.decide('reject');
    await store.previewQuarantine();
    const manifest = store.getSnapshot().quarantine!;
    assert.equal(manifest.items.length, 1);
    await store.commitQuarantine(manifest.id);
    assert.equal((await client.photo(first.id)).quarantined, true);
    assert.ok((await client.quarantinePlans(id)).some(plan => plan.id === manifest.id));
    await store.restoreQuarantine(manifest.id);
    assert.equal((await client.photo(first.id)).quarantined, false);
    await store.deleteProfile('integration');
    assert.ok(received > 0, 'authenticated WebSocket delivered events');
  } finally {
    bus?.stop(); store?.dispose(); child.stdin.end('shutdown\n');
    const kill = setTimeout(() => child.kill(), 5000);
    await exit; clearTimeout(kill);
    await rm(directory, { recursive: true, force: true });
  }
});
