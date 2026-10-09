import { IrisClient } from './client.js';
import type { MarkRequest } from './types.js';
import { EventBus, type ConnectionState, type DaemonEvent, type EventOptions } from './events.js';
import { loadGroupMembers, type GroupSessionState } from './group-session.js';
import { validatedPhotoFilter, ViewPreferencesRepository } from './view-preferences.js';
import { readPhotoAnalysis, type PhotoAnalysisReadout } from './analysis-readout.js';
import type { AcceptCategory, AcceptRequest, Action, Bootstrap, BurstGroup, CacheMigration, CacheStatus, ModelStatusResponse, Photo, PhotoFilter, Profile, Progress, Project, QuarantinePlan, Settings } from './types.js';

interface ProjectScope { id: number; generation: number }
export type { GroupSessionState } from './group-session.js';

export interface ApplicationState {
  bootstrap: Bootstrap | null; projects: Project[]; project: Project | null;
  photos: Photo[]; groups: BurstGroup[]; filter: PhotoFilter; selectedIds: number[];
  focusedId: number | null; reviewOpen: boolean; settings: Settings | null; profiles: Profile[];
  progress: Progress | null; connection: ConnectionState; pending: number; error: string | null;
  quarantine: QuarantinePlan | null; quarantinePlans: QuarantinePlan[];
  reanalysisRequired: boolean;
  /** Detector/occlusion file status only; available is not licensing or accuracy approval. */
  detectorModels: ModelStatusResponse | null;
  groupSession: GroupSessionState | null;
  cache: CacheStatus | null;
  cacheMigrations: CacheMigration[];
  cacheMigration: CacheMigration | null;
}
const initial = (): ApplicationState => ({ bootstrap: null, projects: [], project: null, photos: [], groups: [], filter: { limit: 200, offset: 0 }, selectedIds: [], focusedId: null, reviewOpen: false, settings: null, profiles: [], progress: null, connection: 'disconnected', pending: 0, error: null, quarantine: null, quarantinePlans: [], reanalysisRequired: false, detectorModels: null, groupSession: null, cache: null, cacheMigrations: [], cacheMigration: null });

/** Presentation-independent application state. Server remains decision authority. */
export class IrisStore {
  private state = initial();
  private listeners = new Set<() => void>();
  private generation = 0;
  private refreshSequence = 0;
  private modelSequence = 0;
  private viewRevision = 0;
  private groupGeneration = 0;
  private groupLoadSequence = 0;
  private groupFocusRevision = 0;
  private recentSequence = 0;
  private cacheSequence = 0;
  private bus: EventBus | undefined;
  private eventRefresh: ReturnType<typeof setTimeout> | undefined;
  private eventRefreshGroup = false;
  private queue: Promise<unknown> = Promise.resolve();
  constructor(readonly client: IrisClient, private readonly viewPreferences = new ViewPreferencesRepository()) {}
  getSnapshot = (): ApplicationState => this.state;
  clearError = (): void => { this.update({ error: null }); };
  photoAnalysis = (id: number): PhotoAnalysisReadout | null => {
    const photo = this.state.photos.find(photo => photo.id === id) ?? this.state.groupSession?.photos.find(photo => photo.id === id);
    return photo ? readPhotoAnalysis(photo) : null;
  };
  subscribe = (listener: () => void): (() => void) => { this.listeners.add(listener); return () => this.listeners.delete(listener); };
  private update(patch: Partial<ApplicationState>): void {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) listener();
  }
  async execute<T>(operation: () => Promise<T>, scope?: ProjectScope): Promise<T> {
    this.update({ pending: this.state.pending + 1, error: null });
    try { return await operation(); }
    catch (error) { if (!scope || this.isCurrent(scope)) this.update({ error: error instanceof Error ? error.message : String(error) }); throw error; }
    finally { this.update({ pending: this.state.pending - 1 }); }
  }
  initialize = async (): Promise<void> => this.execute(async () => {
    const sequence = ++this.recentSequence;
    const [bootstrap, projects, profiles] = await Promise.all([this.client.bootstrap(), this.client.projects(), this.client.profiles()]);
    this.update({ bootstrap, profiles, ...(sequence === this.recentSequence ? { projects } : {}) });
  });
  private loadProject = async (operation: () => Promise<Project>, recordRecent: boolean): Promise<void> => this.execute(async () => {
    const generation = ++this.generation;
    this.closeGroup();
    const project = await operation();
    const [settings, detectorModels] = await Promise.all([this.client.settings(project.id), this.client.models(project.id)]);
    if (generation !== this.generation) return;
    const preferences = this.viewPreferences.load(project.root);
    if (recordRecent) ++this.recentSequence;
    this.update({ project, settings, detectorModels, photos: [], groups: [], selectedIds: [], focusedId: null, quarantine: null, quarantinePlans: [], cache: null, cacheMigrations: [], cacheMigration: null, progress: null, reanalysisRequired: false, reviewOpen: preferences.reviewOpen, filter: { ...preferences.filter, offset: 0 }, projects: recordRecent ? [project, ...this.state.projects.filter(p => p.id !== project.id)] : this.state.projects });
    await this.refresh();
  });
  openProject = (root: string): Promise<void> => this.loadProject(() => this.client.openProject(root), true);
  openRecentProject = (id: number): Promise<void> => this.loadProject(() => this.client.openRecentProject(id), true);
  refreshRecentProjects = (): Promise<void> => this.execute(async () => {
    const sequence = ++this.recentSequence;
    const projects = await this.client.projects();
    if (sequence === this.recentSequence) this.update({ projects });
  });
  hideRecentProject = (id: number): Promise<void> => this.execute(async () => {
    await this.client.hideRecentProject(id);
    await this.refreshRecentProjects();
  });
  /** Reconnect to an existing project without treating background recovery as a user open. */
  restoreProject = (id: number): Promise<void> => this.loadProject(() => this.client.project(id), false);
  private projectId(): number { if (!this.state.project) throw new Error('Open a project first'); return this.state.project.id; }
  private scope(): ProjectScope { return { id: this.projectId(), generation: this.generation }; }
  private isCurrent(scope: ProjectScope): boolean { return this.state.project?.id === scope.id && this.generation === scope.generation; }
  private async readDetectorModels(scope: ProjectScope): Promise<void> {
    const sequence = ++this.modelSequence;
    const detectorModels = await this.client.models(scope.id);
    if (this.isCurrent(scope) && sequence === this.modelSequence) this.update({ detectorModels });
  }
  // Hash model files on demand, not on every progress poll.
  refreshDetectorModels = (): Promise<void> => {
    const scope = this.scope();
    return this.execute(() => this.readDetectorModels(scope), scope);
  };
  refresh = async (): Promise<void> => {
    const id = this.state.project?.id;
    if (id === undefined) return;
    const generation = this.generation;
    const sequence = ++this.refreshSequence;
    const [project, photos, groups, progress, quarantinePlans] = await Promise.all([this.client.project(id), this.client.photos(id, this.state.filter), this.client.groups(id), this.client.progress(id), this.client.quarantinePlans(id)]);
    if (generation !== this.generation || sequence !== this.refreshSequence) return;
    const ids = new Set(photos.map(p => p.id));
    // Project-wide counters also cover stale/missing analyses outside this page.
    const reanalysisRequired = project.groups_dirty || project.pending_analysis > 0;
    const session = this.state.groupSession;
    if (session) {
      const group = groups.find(group => group.id === session.groupId);
      if (reanalysisRequired || progress.kind === 'analysis' && ['running', 'paused'].includes(progress.state) || !group || group.member_photo_ids.join(',') !== session.memberIds.join(',')) this.closeGroup();
      else {
        const refreshed = new Map(photos.map(photo => [photo.id, photo]));
        this.update({ groupSession: { ...session, photos: session.photos.map(photo => refreshed.get(photo.id) ?? photo) } });
      }
    }
    this.update({ project, photos, groups: project.groups_dirty ? [] : groups, progress, quarantinePlans, reanalysisRequired, selectedIds: this.state.selectedIds.filter(id => ids.has(id)), focusedId: this.state.focusedId !== null && ids.has(this.state.focusedId) ? this.state.focusedId : photos[0]?.id ?? null });
  };
  setFilter = async (filter: PhotoFilter): Promise<void> => this.execute(async () => {
    const valid = validatedPhotoFilter(filter, true);
    ++this.viewRevision;
    this.update({ filter: { ...valid, offset: valid.offset ?? 0 }, selectedIds: [] });
    this.saveViewPreferences();
    await this.refresh();
  });
  select(ids: number[]): void {
    ++this.viewRevision;
    const available = new Set(this.state.photos.map(p => p.id));
    this.update({ selectedIds: [...new Set(ids)].filter(id => available.has(id)) });
  }
  focus(id: number): void {
    if (this.state.photos.some(p => p.id === id)) { ++this.viewRevision; this.update({ focusedId: id }); }
  }
  private sameView(scope: ProjectScope, revision: number): boolean { return this.isCurrent(scope) && revision === this.viewRevision; }
  private async loadPage(scope: ProjectScope, revision: number, offset: number, index: number): Promise<boolean> {
    const filter = { ...this.state.filter, offset };
    const photos = await this.client.photos(scope.id, filter);
    if (!this.sameView(scope, revision) || !photos.length) return false;
    // A progress poll issued for the old page must not replace this page.
    ++this.refreshSequence;
    this.update({ filter, photos, selectedIds: [], focusedId: photos[Math.min(index, photos.length - 1)]!.id });
    return true;
  }
  private async navigate(delta: number, scope: ProjectScope, revision: number): Promise<void> {
    if (!this.sameView(scope, revision) || !this.state.photos.length) return;
    const index = Math.max(0, this.state.photos.findIndex(p => p.id === this.state.focusedId));
    const target = index + delta;
    if (target >= 0 && target < this.state.photos.length) {
      this.update({ focusedId: this.state.photos[target]!.id, selectedIds: [] });
      return;
    }
    const limit = this.state.filter.limit ?? 200;
    const offset = this.state.filter.offset ?? 0;
    if (target < 0 && offset === 0 || target >= this.state.photos.length && this.state.photos.length < limit) return;
    const absolute = Math.max(0, offset + target);
    const pageOffset = Math.floor(absolute / limit) * limit;
    await this.loadPage(scope, revision, pageOffset, absolute - pageOffset);
  }
  move = (delta: number): Promise<void> => {
    if (!Number.isSafeInteger(delta)) return Promise.reject(new Error('Navigation delta must be an integer'));
    const scope = this.scope();
    const revision = this.viewRevision;
    return this.enqueue(() => this.navigate(delta, scope, revision), scope);
  };
  private saveViewPreferences(): void {
    if (this.state.project) this.viewPreferences.save(this.state.project.root, { filter: this.state.filter, reviewOpen: this.state.reviewOpen });
  }
  toggleReview(): void { this.update({ reviewOpen: !this.state.reviewOpen }); this.saveViewPreferences(); }
  private enqueue(operation: () => Promise<void>, scope?: ProjectScope): Promise<void> {
    const next = this.queue.then(async () => {
      // Skip retired commands before execute() can clear the new view's error.
      if (scope && !this.isCurrent(scope)) return;
      await this.execute(operation, scope);
    });
    this.queue = next.catch(() => undefined);
    return next;
  }
  decide = (action: Action, advance = false): Promise<void> => {
    const scope = this.scope();
    const revision = this.viewRevision;
    return this.enqueue(async () => {
      if (!this.sameView(scope, revision)) return;
      const ids = this.state.selectedIds.length ? this.state.selectedIds : this.state.focusedId === null ? [] : [this.state.focusedId];
      if (!ids.length) return;
      const before = this.state.photos;
      const focusedId = this.state.focusedId;
      const index = before.findIndex(p => p.id === focusedId);
      await this.client.decide(scope.id, ids, action);
      if (!this.isCurrent(scope)) return;
      // Linked capture variants may include members outside the library page.
      await this.refreshWithGroup();
      if (!advance || !this.sameView(scope, revision)) return;
      const visible = new Set(this.state.photos.map(photo => photo.id));
      const next = before.slice(index + 1).find(photo => visible.has(photo.id));
      if (next) { this.update({ focusedId: next.id, selectedIds: [] }); return; }
      if (focusedId !== null && visible.has(focusedId)) {
        this.update({ focusedId });
        await this.navigate(1, scope, revision);
      } else if (this.state.photos.length) {
        // Under a pending-only filter, the decided row disappears. Continue at
        // its shifted position, including a successor pulled in from next page.
        const preceding = before.slice(0, Math.max(0, index)).filter(photo => visible.has(photo.id)).length;
        this.update({ focusedId: this.state.photos[Math.min(preceding, this.state.photos.length - 1)]!.id, selectedIds: [] });
      } else if ((this.state.filter.offset ?? 0) > 0) {
        const limit = this.state.filter.limit ?? 200;
        await this.loadPage(scope, revision, Math.max(0, (this.state.filter.offset ?? 0) - limit), limit - 1);
      }
    }, scope);
  };
  undo = (): Promise<void> => {
    const scope = this.scope();
    return this.enqueue(async () => { await this.client.undo(scope.id); if (this.isCurrent(scope)) await this.refreshWithGroup(); }, scope);
  };
  mark = (fields: Omit<MarkRequest, 'photo_ids'>, ids?: number[]): Promise<void> => {
    const scope = this.scope();
    const focused = this.state.groupSession?.focusedId ?? this.state.focusedId;
    const photo_ids = ids ?? (this.state.groupSession ? (focused === null ? [] : [focused])
      : this.state.selectedIds.length ? [...this.state.selectedIds] : focused === null ? [] : [focused]);
    const request = structuredClone({ ...fields, photo_ids });
    return this.enqueue(async () => {
      if (!this.isCurrent(scope) || !request.photo_ids.length) return;
      await this.client.mark(scope.id, request);
      if (this.isCurrent(scope)) await this.refreshWithGroup();
    }, scope);
  };
  /** Omitted options retain whole-project acceptance; explicit IDs bound the operation. */
  accept = (options?: AcceptRequest): Promise<void> => {
    const scope = this.scope();
    const request = options === undefined ? undefined : structuredClone(options);
    return this.enqueue(async () => {
      if (!this.isCurrent(scope)) return;
      await this.client.accept(scope.id, request);
      if (this.isCurrent(scope)) await this.refreshWithGroup();
    }, scope);
  };
  /** Captures only the loaded page, never every photo matching its filter. */
  acceptVisible = (category: AcceptCategory = 'all'): Promise<void> => this.accept({ photo_ids: this.state.photos.map(photo => photo.id), category });
  acceptSelected = (category: AcceptCategory = 'all'): Promise<void> => this.accept({ photo_ids: [...this.state.selectedIds], category });
  private groupCurrent(scope: ProjectScope, generation: number): boolean { return this.isCurrent(scope) && this.groupGeneration === generation && this.state.groupSession !== null; }
  closeGroup = (): void => { ++this.groupGeneration; ++this.groupLoadSequence; this.update({ groupSession: null }); };
  private requireGroup(): GroupSessionState {
    const session = this.state.groupSession;
    if (!session || session.loading || this.state.reanalysisRequired) throw new Error('Open a current, fully loaded group first');
    return session;
  }
  private async readGroup(scope: ProjectScope, generation: number): Promise<void> {
    const session = this.state.groupSession;
    if (!session || !this.groupCurrent(scope, generation)) return;
    const sequence = ++this.groupLoadSequence;
    const current = () => this.groupCurrent(scope, generation) && sequence === this.groupLoadSequence;
    this.update({ groupSession: { ...session, loading: true } });
    try {
      const photos = await loadGroupMembers(this.client, scope.id, session.memberIds, current);
      if (!current()) return;
      // Recheck project/group authority after member reads, including off-page members.
      await this.refresh();
      if (!current()) return;
      const page = new Map(this.state.photos.map(photo => [photo.id, photo]));
      this.update({ groupSession: { ...session, photos: photos.map(photo => page.get(photo.id) ?? photo), loading: false } });
    } catch (error) {
      if (!current()) return;
      this.closeGroup();
      throw error;
    }
  }
  openGroup = (groupId: string): Promise<void> => {
    const scope = this.scope();
    return this.execute(async () => {
      const group = this.state.groups.find(group => group.id === groupId);
      if (this.state.reanalysisRequired || this.state.progress?.kind === 'analysis' && ['running', 'paused'].includes(this.state.progress.state)) throw new Error('Rebuild analysis before opening a group');
      if (!group || group.project_id !== scope.id || !group.member_photo_ids.length) throw new Error('Group is not available in the current project');
      const memberIds = [...new Set(group.member_photo_ids)];
      const generation = ++this.groupGeneration;
      this.update({ groupSession: { groupId, memberIds, photos: [], focusedId: memberIds[0]!, comparisonIds: memberIds.slice(0, 2), loading: true } });
      await this.readGroup(scope, generation);
    }, scope);
  };
  refreshGroup = (): Promise<void> => {
    const scope = this.scope();
    const generation = this.groupGeneration;
    return this.execute(() => this.readGroup(scope, generation), scope);
  };
  private refreshWithGroup = async (): Promise<void> => {
    const project = this.state.project;
    if (!project) return;
    const scope = this.scope();
    await this.refresh();
    if (this.isCurrent(scope) && this.state.groupSession) await this.readGroup(scope, this.groupGeneration);
  };
  focusGroupPhoto = (id: number): void => {
    const session = this.requireGroup();
    if (!session.memberIds.includes(id)) throw new Error('Photo is not a member of the active group');
    ++this.groupFocusRevision;
    this.update({ groupSession: { ...session, focusedId: id } });
  };
  setComparisonCandidates = (ids: number[]): void => {
    const session = this.requireGroup();
    const comparisonIds = [...new Set(ids)];
    if (comparisonIds.some(id => !session.memberIds.includes(id))) throw new Error('Comparison candidates must belong to the active group');
    this.update({ groupSession: { ...session, comparisonIds } });
  };
  moveGroup = async (delta: number): Promise<void> => {
    if (!Number.isSafeInteger(delta)) throw new Error('Navigation delta must be an integer');
    const session = this.requireGroup();
    const index = this.state.groups.findIndex(group => group.id === session.groupId);
    const target = this.state.groups[index + delta];
    if (target && target.id !== session.groupId) await this.openGroup(target.id);
  };
  moveGroupPhoto = (delta: number): void => {
    if (!Number.isSafeInteger(delta)) throw new Error('Navigation delta must be an integer');
    const session = this.requireGroup();
    const index = session.memberIds.indexOf(session.focusedId!);
    const target = session.memberIds[index + delta];
    if (target !== undefined) this.focusGroupPhoto(target);
  };
  acceptGroup = (category: AcceptCategory = 'all'): Promise<void> => {
    const session = this.requireGroup();
    const scope = this.scope();
    const generation = this.groupGeneration;
    const photo_ids = [...session.memberIds];
    return this.enqueue(async () => {
      if (!this.groupCurrent(scope, generation)) return;
      await this.client.accept(scope.id, { photo_ids, category });
      if (this.groupCurrent(scope, generation)) await this.readGroup(scope, generation);
    }, scope);
  };
  decideGroup = (action: Action, advance = false): Promise<void> => {
    this.requireGroup();
    const scope = this.scope();
    const generation = this.groupGeneration;
    const revision = this.groupFocusRevision;
    return this.enqueue(async () => {
      if (!this.groupCurrent(scope, generation) || revision !== this.groupFocusRevision) return;
      const session = this.requireGroup();
      const index = session.memberIds.indexOf(session.focusedId!);
      await this.client.decide(scope.id, [session.focusedId!], action);
      if (!this.groupCurrent(scope, generation)) return;
      await this.readGroup(scope, generation);
      if (!advance || !this.groupCurrent(scope, generation) || revision !== this.groupFocusRevision) return;
      const current = this.requireGroup();
      const next = current.memberIds[index + 1];
      if (next !== undefined) this.update({ groupSession: { ...current, focusedId: next } });
      else await this.moveGroup(1);
    }, scope);
  };
  run = (operation: 'scan' | 'analyze' | 'cancel' | 'pause' | 'resume'): Promise<void> => {
    const scope = this.scope();
    if (operation === 'analyze' || operation === 'scan') this.closeGroup();
    return this.execute(async () => { await this.client.run(scope.id, operation); if (this.isCurrent(scope)) await this.refresh(); }, scope);
  };
  reanalyze = (): Promise<void> => this.run('analyze');
  retryFailed = (): Promise<void> => {
    const scope = this.scope();
    this.closeGroup();
    return this.execute(async () => { await this.client.retryFailed(scope.id); if (this.isCurrent(scope)) await this.refresh(); }, scope);
  };
  retryScan = (): Promise<void> => {
    const scope = this.scope();
    this.closeGroup();
    return this.execute(async () => { await this.client.retryScan(scope.id); if (this.isCurrent(scope)) await this.refresh(); }, scope);
  };
  private updateProjectSettings(operation: (id: number) => Promise<Settings>): Promise<void> {
    const scope = this.scope();
    // Serialize writes, including profile applications, so response order and
    // server commit order cannot disagree with the user's command order.
    return this.enqueue(async () => {
      const settings = await operation(scope.id);
      if (!this.isCurrent(scope)) return;
      // Invalidate in-flight refreshes and old groups before refetching server state.
      ++this.refreshSequence;
      ++this.modelSequence;
      this.closeGroup();
      // The write has committed. Until the refresh succeeds, cached analysis is
      // historical even if a follow-up request fails; preserve it with stale status.
      const photos: Photo[] = this.state.photos.map(photo => ({ ...photo, analysis_status: photo.analysis ? 'stale' : 'missing' }));
      this.update({ settings, detectorModels: null, groups: [], photos, reanalysisRequired: true });
      await Promise.all([this.refresh(), this.readDetectorModels(scope)]);
    }, scope);
  }
  saveSettings = (settings: Settings): Promise<void> => {
    const request = structuredClone(settings);
    return this.updateProjectSettings(id => this.client.saveSettings(id, request));
  };
  saveProfile = (profile: Profile): Promise<void> => this.execute(async () => { await this.client.saveProfile(profile); this.update({ profiles: await this.client.profiles() }); });
  deleteProfile = (name: string): Promise<void> => this.execute(async () => { await this.client.deleteProfile(name); this.update({ profiles: await this.client.profiles() }); });
  applyProfile = (name: string): Promise<void> => this.updateProjectSettings(id => this.client.applyProfile(id, name));
  previewQuarantine = (): Promise<void> => {
    const scope = this.scope();
    return this.execute(async () => { const quarantine = await this.client.quarantinePreview(scope.id); if (this.isCurrent(scope)) this.update({ quarantine }); }, scope);
  };
  reviewQuarantinePlan(manifestId: string): void {
    const plan = this.state.quarantinePlans.find(plan => plan.id === manifestId);
    if (!plan) throw new Error('Quarantine manifest is not in the current project');
    this.update({ quarantine: plan });
  }
  commitQuarantine = (confirmedManifestId: string): Promise<void> => {
    const scope = this.scope();
    return this.execute(async () => {
      if (!this.state.quarantine || this.state.quarantine.id !== confirmedManifestId) throw new Error('Preview and confirm this quarantine manifest first');
      const quarantine = await this.client.quarantineCommit(scope.id, confirmedManifestId);
      if (!this.isCurrent(scope)) return;
      this.update({ quarantine }); await this.refresh();
    }, scope);
  };
  restoreQuarantine = (manifestId: string): Promise<void> => {
    const scope = this.scope();
    return this.execute(async () => {
      let quarantine: QuarantinePlan;
      try { quarantine = await this.client.quarantineRestore(scope.id, manifestId); }
      catch (error) { if (this.isCurrent(scope)) { try { await this.refresh(); } catch { /* Keep the original recovery error. */ } } throw error; }
      if (!this.isCurrent(scope)) return;
      this.update({ quarantine }); await this.refresh();
    }, scope);
  };
  private async readCache(scope: ProjectScope): Promise<void> {
    const sequence = ++this.cacheSequence;
    const [cache, cacheMigrations] = await Promise.all([this.client.cache(scope.id), this.client.cacheMigrations(scope.id)]);
    if (!this.isCurrent(scope) || sequence !== this.cacheSequence) return;
    const selected = this.state.cacheMigration?.id;
    this.update({ cache, cacheMigrations, cacheMigration: cacheMigrations.find(migration => migration.id === selected) ?? null });
  }
  refreshCache = (): Promise<void> => {
    const scope = this.scope();
    return this.execute(() => this.readCache(scope), scope);
  };
  migrateCache = (destination: string): Promise<void> => {
    const scope = this.scope();
    return this.execute(async () => {
      let cache: CacheStatus;
      try { cache = await this.client.migrateCache(scope.id, destination); }
      catch (error) {
        if (this.isCurrent(scope)) { try { await this.readCache(scope); } catch { /* Keep the original write error. */ } }
        throw error;
      }
      if (!this.isCurrent(scope)) return;
      ++this.cacheSequence;
      this.update({ cache, cacheMigration: null });
      await Promise.all([this.refresh(), this.readCache(scope)]);
    }, scope);
  };
  reviewCacheMigration = (id: string): void => {
    const migration = this.state.cacheMigrations.find(migration => migration.id === id && migration.project_id === this.state.project?.id);
    if (!migration) throw new Error('Cache migration is not in the current project history');
    this.update({ cacheMigration: migration });
  };
  clearCacheMigrationReview = (): void => { this.update({ cacheMigration: null }); };
  cleanupPreviousCache = (confirmedMigrationId: string): Promise<void> => {
    const scope = this.scope();
    return this.execute(async () => {
      if (this.state.cacheMigration?.id !== confirmedMigrationId || this.state.cacheMigration.project_id !== scope.id) throw new Error('Review and confirm this cache migration first');
      let cacheMigration: CacheMigration;
      try { cacheMigration = await this.client.cleanupPreviousCache(scope.id, confirmedMigrationId); }
      catch (error) {
        if (this.isCurrent(scope)) { try { await this.readCache(scope); } catch { /* Keep the original write error. */ } }
        throw error;
      }
      if (!this.isCurrent(scope)) return;
      ++this.cacheSequence;
      if (this.state.cacheMigration?.id === confirmedMigrationId) this.update({ cacheMigration });
      await this.readCache(scope);
    }, scope);
  };
  connect(options: Partial<Pick<EventOptions, 'createSocket' | 'retryMs' | 'pollMs'>> = {}): void {
    this.bus?.stop();
    this.bus = new EventBus(this.client.transport.session, {
      ...options,
      synchronize: () => this.state.connection === 'connected' ? this.refresh() : this.refreshWithGroup(),
      onEvent: this.handleEvent,
      onState: connection => {
        this.update({ connection });
        if (connection === 'connected') this.scheduleRefresh(true);
      },
      onError: error => this.update({ error: error instanceof Error ? error.message : String(error) }),
    });
    this.bus.start();
  }
  private scheduleRefresh(group = false): void {
    this.eventRefreshGroup ||= group;
    if (this.eventRefresh !== undefined) return;
    this.eventRefresh = setTimeout(() => {
      this.eventRefresh = undefined;
      const refresh = this.eventRefreshGroup ? this.refreshWithGroup : this.refresh;
      this.eventRefreshGroup = false;
      void refresh().catch(error => this.update({ error: String(error) }));
    }, 50);
  }
  private handleEvent = (event: DaemonEvent): void => {
    if (event.event === 'resync') { this.scheduleRefresh(true); return; }
    if (event.project_id !== this.state.project?.id) return;
    if (event.event === 'scan:progress' || event.event === 'analysis:stage') {
      if (typeof event.data === 'object' && event.data !== null) {
        const progress = event.data as Progress;
        if (progress.kind === 'analysis' && ['running', 'paused'].includes(progress.state)) this.closeGroup();
        this.update({ progress });
        if (['completed', 'cancelled', 'failed'].includes(progress.state)) this.scheduleRefresh();
      }
    }
    if (event.event === 'session:changed' || event.event === 'verdict:updated') this.scheduleRefresh(true);
  };
  disconnect(): void { this.bus?.stop(); this.bus = undefined; clearTimeout(this.eventRefresh); this.eventRefresh = undefined; this.eventRefreshGroup = false; }
  dispose(): void { this.disconnect(); ++this.generation; this.listeners.clear(); }
}
