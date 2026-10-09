import { Transport } from './transport.js';
import type { MarkRequest } from './types.js';
import type { AcceptRequest, Action, Bootstrap, BurstGroup, CacheMigration, CacheStatus, DecisionBatch, ExportReport, ModelStatusResponse, Photo, PhotoFilter, Profile, Progress, Project, QuarantinePlan, Settings } from './types.js';

const projectPath = (id: number) => `/api/v1/projects/${id}`;
const namePath = (name: string) => `/api/v1/profiles/${encodeURIComponent(name)}`;

export class IrisClient {
  constructor(readonly transport: Transport) {}
  bootstrap = () => this.transport.request<Bootstrap>('GET', '/api/v1/bootstrap');
  contract = () => this.transport.request<Record<string, unknown>>('GET', '/api/v1/openapi.json');
  projects = () => this.transport.request<Project[]>('GET', '/api/v1/projects');
  openProject = (root: string) => this.transport.request<Project>('POST', '/api/v1/projects', { root, auto_device: true });
  gpuDevices = () => this.transport.request<import('./types.js').GpuDevices>('GET', '/api/v1/devices/gpu');
  optionalModels = () => this.transport.request<import('./types.js').OptionalModel[]>('GET', '/api/v1/models/optional');
  modelInstallStatus = () => this.transport.request<import('./types.js').InstallProgress>('GET', '/api/v1/models/install');
  installModel = (request: import('./types.js').InstallRequest) => this.transport.request<import('./types.js').InstallProgress>('POST', '/api/v1/models/install', request);
  cancelModelInstall = () => this.transport.request<import('./types.js').InstallProgress>('POST', '/api/v1/models/install/cancel');
  openRecentProject = (id: number) => this.transport.request<Project>('POST', `${projectPath(id)}/open`);
  hideRecentProject = (id: number) => this.transport.request<Project>('POST', `${projectPath(id)}/hide`);
  project = (id: number) => this.transport.request<Project>('GET', projectPath(id));
  photos = (id: number, filter: PhotoFilter = {}, signal?: AbortSignal) => {
    const query = new URLSearchParams();
    for (const [key, value] of Object.entries(filter)) if (value !== undefined) query.set(key, String(value));
    return this.transport.request<Photo[]>('GET', `${projectPath(id)}/photos?${query}`, undefined, signal);
  };
  photo = (id: number) => this.transport.request<Photo>('GET', `/api/v1/photos/${id}`);
  media = (id: number, kind: 'thumb' | 'preview' | 'original', signal?: AbortSignal) => this.transport.blob(`/api/v1/photos/${id}/${kind}`, signal);
  groups = (id: number) => this.transport.request<BurstGroup[]>('GET', `${projectPath(id)}/groups`);
  progress = (id: number) => this.transport.request<Progress>('GET', `${projectPath(id)}/progress`);
  run = (id: number, operation: 'scan' | 'analyze' | 'cancel' | 'pause' | 'resume') => this.transport.request<Progress>('POST', `${projectPath(id)}/${operation}`);
  retryFailed = (id: number) => this.transport.request<Progress>('POST', `${projectPath(id)}/analyze`, { retry_failed_only: true });
  retryScan = (id: number) => this.transport.request<Progress>('POST', `${projectPath(id)}/scan`, { retry_failed_only: true });
  decide = (id: number, photo_ids: number[], action: Action, link_variants = true) => this.transport.request<DecisionBatch>('POST', `${projectPath(id)}/decisions`, { photo_ids, action, link_variants });
  undo = (id: number) => this.transport.request<DecisionBatch | null>('POST', `${projectPath(id)}/undo`);
  mark = (id: number, request: MarkRequest) => this.transport.request<DecisionBatch>('POST', `${projectPath(id)}/marks`, request);
  accept = (id: number, options?: AcceptRequest) => this.transport.request<DecisionBatch>('POST', `${projectPath(id)}/accept`, options);
  exportXmp = (id: number, scope = 'keep', overwrite = false) => this.transport.request<ExportReport>('POST', `${projectPath(id)}/export/xmp`, { scope, overwrite });
  exportCopy = (id: number, destination: string, scope = 'keep') => this.transport.request<ExportReport>('POST', `${projectPath(id)}/export/copy`, { destination, scope });
  exportCsv = (id: number, destination: string) => this.transport.request<ExportReport>('POST', `${projectPath(id)}/export/csv`, { destination });
  importCsv = (id: number, source: string) => this.transport.request<DecisionBatch>('POST', `${projectPath(id)}/import/csv`, { source });
  quarantinePlans = (id: number) => this.transport.request<QuarantinePlan[]>('GET', `${projectPath(id)}/quarantine`);
  quarantinePreview = (id: number) => this.transport.request<QuarantinePlan>('POST', `${projectPath(id)}/quarantine/preview`);
  quarantineCommit = (id: number, manifest_id: string) => this.transport.request<QuarantinePlan>('POST', `${projectPath(id)}/quarantine/commit`, { manifest_id });
  quarantineRestore = (id: number, manifest_id: string) => this.transport.request<QuarantinePlan>('POST', `${projectPath(id)}/quarantine/restore`, { manifest_id });
  settings = (id: number) => this.transport.request<Settings>('GET', `/api/v1/settings?project_id=${id}`);
  models = (id: number) => this.transport.request<ModelStatusResponse>('GET', `/api/v1/models?project_id=${id}`);
  saveSettings = (id: number, settings: Settings) => this.transport.request<Settings>('PUT', `/api/v1/settings?project_id=${id}`, settings);
  profiles = () => this.transport.request<Profile[]>('GET', '/api/v1/profiles');
  saveProfile = (profile: Profile) => this.transport.request<Profile>('POST', '/api/v1/profiles', profile);
  deleteProfile = (name: string) => this.transport.request<unknown>('DELETE', namePath(name));
  applyProfile = (id: number, name: string) => this.transport.request<Settings>('POST', `${namePath(name)}/apply`, { project_id: id });
  estimateProfile = (id: number, name: string) => this.transport.request<Record<string, unknown>>('POST', `${projectPath(id)}/profiles/${encodeURIComponent(name)}/estimate`);
  cache = (id: number) => this.transport.request<CacheStatus>('GET', `${projectPath(id)}/cache`);
  cleanupCache = (id: number) => this.transport.request<CacheStatus>('POST', `${projectPath(id)}/cache/cleanup`);
  migrateCache = (id: number, destination: string) => this.transport.request<CacheStatus>('POST', `${projectPath(id)}/cache/migrate`, { destination });
  cacheMigrations = (id: number) => this.transport.request<CacheMigration[]>('GET', `${projectPath(id)}/cache/migrations`);
  cleanupPreviousCache = (id: number, migrationId: string) => this.transport.request<CacheMigration>('POST', `${projectPath(id)}/cache/migrations/${encodeURIComponent(migrationId)}/cleanup`);
}
