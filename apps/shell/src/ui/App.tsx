import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from 'react';
import type { IrisStore } from '../store.js';
import type { CommandSystem } from '../commands.js';
import type { Action, Photo, Project } from '../types.js';
import { useIris } from '../react.js';
import { readPhotoAnalysis } from '../analysis-readout.js';
import { selectProjectFolder } from '../native-dialog.js';
import { Button, Dialog, PhotoImage } from './components.js';
import { loadAppearance, saveAppearance, type Appearance } from './preferences.js';
import { AnalysisSettings } from './AnalysisSettings.js';
import { Icon, type IconName } from './Icon.js';
import { FileWorkflows } from './FileWorkflows.js';
import { CacheSettings } from './CacheSettings.js';
import { MarkTools, colorNames } from './MarkTools.js';
import { PhotoFilters, filterLabels, filterValue } from './PhotoFilters.js';
import { TaskBar } from './TaskBar.js';
import { ModelTaskBar } from './ModelTaskBar.js';
import { LibraryNav } from './LibraryNav.js';
import { ZoomViewer, type FaceBox, type ZoomApi } from './ZoomViewer.js';
import { useFullscreen } from './fullscreen.js';
import { Histogram } from './Histogram.js';
import { Panel } from './Panel.js';
import { workspaceShortcut } from './workspace-shortcuts.js';
import { GALLERY_MIN, GALLERY_LOUPE, useGalleryMotion } from './gallery-motion.js';
import './app.css';

const decisionNames: Record<Action, string> = { pending: '未标记', keep: '保留', flag: '已标记', reject: '标记移除' };
const decisionIcons: Record<Exclude<Action, 'pending'>, IconName> = { keep: 'done', flag: 'flag', reject: 'close' };
const verdictNames = { recommend: '建议保留', review: '需要复核', reject_suggest: '建议移除' };
const componentNames: Record<string, string> = { eyes: '眼部状态', sharpness: '清晰度', face: '面部质量', exposure: '曝光与构图', smile: '表情' };
const tallyOrder = ['keep', 'flag', 'reject', 'pending'] as const;

function readStored(key: string): string | null { try { return localStorage.getItem(key); } catch { return null; } }
function usePersisted<T extends number | boolean>(key: string, fallback: NoInfer<T>, valid: (value: T) => boolean = () => true) {
  const [value, setValue] = useState<T>(() => {
    const raw = readStored(key);
    if (raw === null) return fallback;
    const parsed = (typeof fallback === 'boolean' ? raw === '1' : Number(raw)) as T;
    return valid(parsed) ? parsed : fallback;
  });
  useEffect(() => { try { localStorage.setItem(key, typeof value === 'boolean' ? (value ? '1' : '0') : String(value)); } catch { /* Session value still applies. */ } }, [key, value]);
  return [value, setValue] as const;
}

function Stars({ rating }: { rating: number }) {
  return <span className="stars" role="img" aria-label={`${rating} 星`}>{Array.from({ length: 5 }, (_, index) => <Icon key={index} name="star" className={index < rating ? 'on' : 'off'} />)}</span>;
}
function DecisionBadge({ decision }: { decision: Action }) {
  if (decision === 'pending') return null;
  return <span className={`badge badge-${decision}`}><Icon name={decisionIcons[decision]} />{decisionNames[decision]}</span>;
}
function ScoreChip({ photo }: { photo: Photo }) {
  const analysis = readPhotoAnalysis(photo);
  return analysis.score === null ? null : <span className={`score-chip verdict-${analysis.verdict ?? 'none'}`} title={analysis.verdict ? verdictNames[analysis.verdict] : undefined}>{Math.round(analysis.score)}</span>;
}
function formatBytes(bytes: number) { return bytes >= 1048576 ? `${(bytes / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1024))} KB`; }
function formatTaken(value: string | null | undefined) {
  if (!value) return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' });
}

/** Face rectangles from current analysis, normalized against the analyzed raster. */
function faceBoxes(photo: Photo | undefined): FaceBox[] {
  const current = photo ? readPhotoAnalysis(photo).current : null;
  if (!current || !current.width || !current.height) return [];
  return current.faces.filter(face => face.bbox.length === 4 && face.bbox.every(Number.isFinite)).map(face => {
    const [x, y, w, h] = face.bbox as [number, number, number, number];
    const eyes = [face.left_eye?.state, face.right_eye?.state];
    const closed = eyes.includes('closed'), uncertain = eyes.includes('uncertain') || eyes.includes(undefined);
    return { x: x / current.width, y: y / current.height, w: w / current.width, h: h / current.height,
      label: closed ? '闭眼' : uncertain ? '眼部不确定' : '睁眼', tone: closed ? 'bad' : uncertain ? 'warn' : 'good' };
  });
}

export function App({ store, commands }: { store: IrisStore; commands: CommandSystem }) {
  const state = useIris(store);
  const [page, setPage] = useState<'projects' | 'photos' | 'settings'>(() => state.project ? 'photos' : 'projects');
  const [appearance, setAppearance] = useState<Appearance>(loadAppearance);
  const [error, setError] = useState<string | null>(null);
  const [scanRoot, setScanRoot] = useState<string | null>(null);
  const [filterOpen, setFilterOpen] = useState(false);
  const [inaccessible, setInaccessible] = useState<{ project: Project; reason: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [tasksOpen, setTasksOpen] = useState(false);
  const taskPanel = useRef<HTMLDivElement>(null);
  const taskTrigger = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!tasksOpen) return;
    taskPanel.current?.querySelector<HTMLButtonElement>('button')?.focus();
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && !taskPanel.current?.contains(event.target) && !taskTrigger.current?.contains(event.target)) setTasksOpen(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { event.preventDefault(); setTasksOpen(false); taskTrigger.current?.focus(); }
    };
    document.addEventListener('pointerdown', outside);
    document.addEventListener('keydown', escape);
    return () => { document.removeEventListener('pointerdown', outside); document.removeEventListener('keydown', escape); };
  }, [tasksOpen]);
  const [settingsDirty, setSettingsDirty] = useState(false);
  const [closeRequested, setCloseRequested] = useState(false);
  const [leaveTarget, setLeaveTarget] = useState<'projects' | 'photos' | null>(null);
  const [savingLeave, setSavingLeave] = useState(false);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const saveSettings = useRef<(() => Promise<boolean>) | null>(null);
  const registerSave = useCallback((save: (() => Promise<boolean>) | null) => { saveSettings.current = save; }, []);
  const workspace = useRef<HTMLElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  const zoomApi = useRef<ZoomApi | null>(null);
  const [zoomReadout, setZoomReadout] = useState({ percent: 100, fitted: true });
  const onZoomChange = useCallback((percent: number, fitted: boolean) => setZoomReadout({ percent, fitted }), []);
  const fullscreen = useFullscreen(stageRef);
  const [tileSize, setTileSize] = usePersisted<number>('iris:tile-size:v2', 210, value => value >= GALLERY_MIN && value < GALLERY_LOUPE);
  const [leftOpen, setLeftOpen] = usePersisted<boolean>('iris:left-panel:v1', true);
  const [rightOpen, setRightOpen] = usePersisted<boolean>('iris:right-panel:v1', true);
  const [stripOpen, setStripOpen] = usePersisted<boolean>('iris:filmstrip:v1', true);
  const [showFaces, setShowFaces] = usePersisted<boolean>('iris:face-overlay:v1', false);
  const [tileInfo, setTileInfo] = usePersisted<boolean>('iris:tile-info:v1', true);
  const [wide, setWide] = useState(() => window.matchMedia('(min-width: 1151px)').matches);
  const [libraryDrawer, setLibraryDrawer] = useState(false);
  const libraryOpen = wide ? leftOpen : libraryDrawer;
  const toggleLibrary = () => wide ? setLeftOpen(!leftOpen) : setLibraryDrawer(!libraryDrawer);
  useEffect(() => {
    const query = window.matchMedia('(min-width: 1151px)');
    const update = () => { setWide(query.matches); setLibraryDrawer(false); };
    query.addEventListener('change', update);
    return () => query.removeEventListener('change', update);
  }, []);

  const navigate = (target: 'projects' | 'photos' | 'settings') => {
    if (settingsDirty && target !== 'settings') setLeaveTarget(target);
    else setPage(target);
  };
  const group = state.groupSession;
  const groupIndex = group ? state.groups.findIndex(item => item.id === group.groupId) : -1;
  const current = group ? group.photos.find(photo => photo.id === group.focusedId) : state.photos.find(photo => photo.id === state.focusedId);
  const attempt = (operation: () => Promise<unknown>) => {
    setError(null);
    store.clearError();
    void operation().catch(reason => setError(reason instanceof Error ? reason.message : String(reason)));
  };
  useEffect(() => { saveAppearance(appearance); }, [appearance]);
  useEffect(() => {
    void window.__TAURI__?.core.invoke('set_unsaved_changes', { dirty: settingsDirty }).catch(reason => setError(`关闭保护暂不可用：${String(reason)}`));
  }, [settingsDirty]);
  useEffect(() => {
    const listening = window.__TAURI__?.event.listen('iris:close-requested', () => setCloseRequested(true));
    void listening?.catch(reason => setError(`关闭保护暂不可用：${String(reason)}`));
    return () => { void listening?.then(unlisten => unlisten()).catch(() => {}); };
  }, []);
  useEffect(() => {
    if (page !== 'photos' || !current) return;
    workspace.current?.querySelectorAll(`[data-photo-id="${current.id}"]`).forEach(node => node.scrollIntoView({ block: 'nearest', inline: 'nearest' }));
  }, [page, current?.id, state.reviewOpen, group?.groupId, stripOpen]);
  const modal = !!scanRoot || filterOpen || shortcutsOpen || tasksOpen;
  useEffect(() => {
    if (page !== 'photos' || modal) return;
    return commands.attach(window, reason => setError(String(reason)));
  }, [commands, page, modal]);
  const viewing = state.reviewOpen || !!group;
  // Digits rate (Lightroom convention); the viewer zooms with + / − / Shift+Z and the HUD.
  const viewKeys = useRef<(event: KeyboardEvent) => void>(() => {});
  viewKeys.current = event => {
    const action = workspaceShortcut(event);
    if (action === null) return;
    const take = () => event.preventDefault();
    if (action === 'panels') {
      take();
      const open = libraryOpen || rightOpen;
      if (wide) setLeftOpen(!open); else setLibraryDrawer(false);
      setRightOpen(!open);
    } else if (action === 'help') { take(); setShortcutsOpen(true); }
    else if (action === 'grid') { take(); setView('grid'); }
    else if (action === 'loupe') { if (current && !group && !state.reviewOpen) { take(); setView('loupe'); } }
    else if (action === 'faces') { if (view === 'loupe' && hasAnalysis) { take(); setShowFaces(value => !value); } }
    else if (action === 'fullscreen') { if (viewing && fullscreen.supported) { take(); void fullscreen.toggle().catch(reason => setError(String(reason))); } }
    else if (typeof action === 'number') { if (current && !group?.loading) { take(); attempt(() => store.mark({ rating: action })); } }
    else if (!group && current) {
      if (action === 'zoomIn') { take(); stepZoom(1.25); }
      else if (action === 'zoomOut') { take(); stepZoom(0.8); }
      else if (action === 'zoomToggle' && zoomApi.current) { take(); if (zoomReadout.fitted) zoomApi.current.actual(); else zoomApi.current.fit(); }
    }
  };
  useEffect(() => {
    if (page !== 'photos' || modal) return;
    const handler = (event: KeyboardEvent) => viewKeys.current(event);
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [page, modal]);

  const openRecent = async (project: Project) => {
    try { await store.openRecentProject(project.id); setInaccessible(null); setPage('photos'); }
    catch (reason) { setInaccessible({ project, reason: reason instanceof Error ? reason.message : String(reason) }); store.clearError(); }
  };
  const pickFolder = async () => {
    const bridge = window.__TAURI__?.core;
    if (!bridge) throw new Error('请在桌面应用中选择文件夹');
    const root = await selectProjectFolder(bridge);
    if (root) setScanRoot(root);
  };
  const startScan = async () => {
    if (!scanRoot || busy) return;
    setBusy(true);
    try { await store.openProject(scanRoot); await store.run('scan'); setScanRoot(null); setPage('photos'); }
    finally { setBusy(false); }
  };
  const progress = state.progress;
  const running = !!progress && ['running', 'paused', 'cancelling'].includes(progress.state);
  const activeFilters = Object.keys(filterLabels).filter(key => state.filter[key as keyof typeof state.filter] !== undefined).length;
  const limit = state.filter.limit ?? 200;
  const offset = state.filter.offset ?? 0;
  const reviewIndex = current ? state.photos.findIndex(photo => photo.id === current.id) : -1;
  const groupPosition = group ? group.memberIds.indexOf(group.focusedId!) : -1;
  const view = group ? 'compare' : state.reviewOpen ? 'loupe' : 'grid';
  const galleryMotion = useGalleryMotion(workspace, view, tileSize, `${page}:${state.project?.id}`);
  const setView = (target: 'grid' | 'loupe') => {
    if (target !== view && current && !group) galleryMotion.capturePhoto(current.id);
    if (group) store.closeGroup();
    if (target === 'loupe') store.select([]);
    if ((target === 'loupe') !== state.reviewOpen) store.toggleReview();
  };
  const resizeGallery = (value: number) => {
    if (!current || group) return;
    if (value >= GALLERY_LOUPE) { setView('loupe'); return; }
    if (view === 'grid') galleryMotion.captureTiles();
    else setView('grid');
    setTileSize(Math.max(GALLERY_MIN, Math.min(GALLERY_LOUPE - 1, value)));
  };
  const stepZoom = (factor: number) => {
    if (view === 'grid') resizeGallery(Math.round(tileSize * factor));
    else if (factor < 1 && zoomReadout.fitted) setView('grid');
    else zoomApi.current?.zoomBy(factor);
  };
  const galleryWheel = useRef<(event: WheelEvent) => void>(() => {});
  galleryWheel.current = event => {
    if (view !== 'grid' || !(event.ctrlKey || event.metaKey) || !current) return;
    event.preventDefault();
    const delta = event.deltaY * (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? 400 : 1);
    resizeGallery(Math.round(tileSize * Math.exp(-Math.max(-150, Math.min(150, delta)) * 0.006)));
  };
  useEffect(() => {
    const node = workspace.current?.querySelector('.canvas');
    if (!node) return;
    const wheel = (event: Event) => galleryWheel.current(event as WheelEvent);
    node.addEventListener('wheel', wheel, { passive: false });
    return () => node.removeEventListener('wheel', wheel);
  }, [page, state.project?.id]);
  const toggleSelected = (id: number, checked: boolean) => store.select(checked ? [...state.selectedIds, id] : state.selectedIds.filter(item => item !== id));
  const clickPhoto = (photo: Photo, index: number, event: { metaKey: boolean; ctrlKey: boolean; shiftKey: boolean }) => {
    if (view === 'grid' && (event.metaKey || event.ctrlKey)) toggleSelected(photo.id, !state.selectedIds.includes(photo.id));
    else if (view === 'grid' && event.shiftKey && state.focusedId !== null) {
      const from = Math.max(0, state.photos.findIndex(item => item.id === state.focusedId));
      const [a, b] = from < index ? [from, index] : [index, from];
      store.select(state.photos.slice(a, b + 1).map(item => item.id));
    } else { store.select([]); store.focus(photo.id); }
  };
  const openLoupe = (photo: Photo) => { if (!state.reviewOpen && !group) galleryMotion.capturePhoto(photo.id); store.select([]); store.focus(photo.id); if (!state.reviewOpen) store.toggleReview(); };
  const counts = useMemo(() => {
    const result = { keep: 0, flag: 0, reject: 0, pending: 0 };
    for (const photo of state.photos) result[photo.decision]++;
    return result;
  }, [state.photos]);
  const faces = useMemo(() => showFaces && view === 'loupe' ? faceBoxes(current) : [], [showFaces, view, current]);
  const hasAnalysis = !!current && readPhotoAnalysis(current).current !== null;
  const bulk = !group && state.selectedIds.length > 0;
  const fullscreenButton = fullscreen.supported && <button type="button" className="hud-button" aria-label={fullscreen.active ? '退出全屏' : '全屏'} title={fullscreen.active ? '退出全屏 (F / Esc)' : '全屏 (F)'} onClick={() => void fullscreen.toggle().catch(reason => setError(String(reason)))}><Icon name={fullscreen.active ? 'fullscreen-exit' : 'fullscreen'} /></button>;

  return <div className="app">
    <header className="titlebar">
      <div className="titlebar-brand"><span className="brand-mark" aria-hidden="true" /><span className="brand">伊人</span></div>
      {state.project && <button type="button" className="crumb" onClick={() => navigate('projects')} title={state.project.root}><Icon name="folder" /><span className="crumb-name">{state.project.name}</span></button>}
      <nav className="modules" aria-label="主导航">
        <button type="button" className="module" aria-current={page === 'projects' ? 'page' : undefined} onClick={() => navigate('projects')}>项目</button>
        <button type="button" className="module" disabled={!state.project} aria-current={page === 'photos' ? 'page' : undefined} onClick={() => navigate('photos')}>选片</button>
        <button type="button" className="module" aria-current={page === 'settings' ? 'page' : undefined} onClick={() => navigate('settings')}>设置</button>
      </nav>
      <div className="titlebar-tools">
        <span className={`connection connection-${state.connection}`} role="status" title={state.connection === 'disconnected' ? '服务连接已断开' : '服务已连接'}><span className="connection-dot" aria-hidden="true" /><span className="visually-hidden">{state.connection === 'disconnected' ? '服务连接已断开' : '服务已连接'}</span></span>
        <button ref={taskTrigger} type="button" className="title-button" title="任务与运行详情" aria-label="任务" aria-expanded={tasksOpen} aria-controls="task-center" onClick={() => setTasksOpen(value => !value)}>{running ? <span className="activity-ring" aria-hidden="true" /> : <Icon name="schedule" />}</button>
        <button type="button" className="title-button" title="键盘快捷键 (?)" aria-label="键盘快捷键" onClick={() => setShortcutsOpen(true)}><Icon name="info" /></button>
      </div>
    </header>
    {(error || state.error) && <div role="alert" className="error-bar"><Icon name="warning" /><span>{error || state.error}</span><span className="spacer" /><Button className="quiet" onClick={() => { setError(null); store.clearError(); }}>关闭提示</Button></div>}
    {state.connection === 'disconnected' && <div role="status" className="error-bar notice"><span className="activity-dot" aria-hidden="true" /><span>服务连接已断开，正在等待恢复</span></div>}

    {page === 'projects' && inaccessible && <main className="page"><div className="page-content state-page">
      <Icon name="warning" className="state-icon" /><h1>文件夹无法访问</h1>
      <div><h2>{inaccessible.project.name}</h2><p className="filename muted">{inaccessible.project.root}</p></div>
      <p className="muted">请检查连接、权限或文件夹位置。</p>
      <div className="row"><Button primary onClick={() => attempt(() => openRecent(inaccessible.project))}><Icon name="refresh" />重试</Button><Button onClick={() => { setInaccessible(null); store.clearError(); }}>返回项目列表</Button></div>
      <details><summary>详细原因</summary><p>{inaccessible.reason}</p></details>
    </div></main>}

    {page === 'projects' && !inaccessible && <main className="page projects-page">
      <div className="page-content projects-content">
        <div className="projects-heading"><h1>项目</h1><Button primary disabled={busy} onClick={() => attempt(pickFolder)}><Icon name="folder" />打开照片文件夹</Button></div>
        <div className="section-heading"><h2>最近项目</h2></div>
        {state.projects.length === 0 ? <p className="project-empty">暂无最近项目</p>
          : <ul className="project-list">{state.projects.map(project => <li key={project.id} className={`project-card ${state.project?.id === project.id ? 'current' : ''}`}>
            <button className="project-open" title={project.root} aria-label={`${project.name}，${project.root}`} aria-current={state.project?.id === project.id ? 'true' : undefined} onClick={() => state.project?.id === project.id ? navigate('photos') : attempt(() => openRecent(project))}>
              <span className="project-glyph" aria-hidden="true"><Icon name="folder" /></span>
              <span className="project-text"><strong>{project.name}</strong></span>
              <Icon name="chevron-right" className="project-chevron" />
            </button>
            <button type="button" className="project-remove" aria-label={`移出列表：${project.name}`} title="移出列表" onClick={() => attempt(() => store.hideRecentProject(project.id))}><Icon name="close" /></button>
          </li>)}</ul>}
      </div>
    </main>}

    {page === 'settings' && <main className="page settings-page"><div className="page-content">
      <header className="page-heading">
        <h1>设置</h1>
        {state.project && <Button className="ghost" onClick={() => navigate('photos')}><Icon name="back" />返回选片</Button>}
      </header>
      <div className="settings-layout">
        <nav className="settings-nav" aria-label="设置分类"><a href="#appearance"><Icon name="tune" />外观</a><a href="#analysis"><Icon name="auto-awesome" />模型与分析</a>{state.project && <a href="#storage"><Icon name="storage" />存储与缓存</a>}</nav>
        <div className="settings-body">
          <section id="appearance" className="settings-section"><h2>外观</h2>
            <div className="setting-row"><span>字号</span><div className="segmented" role="group" aria-label="字号">{([['small', '小'], ['medium', '中'], ['large', '大']] as const).map(([value, label]) => <Button key={value} aria-pressed={appearance.size === value} onClick={() => setAppearance({ ...appearance, size: value })}>{label}</Button>)}</div></div>
            <div className="setting-row"><span>界面密度</span><div className="segmented" role="group" aria-label="界面密度">{([['compact', '紧凑'], ['standard', '标准'], ['relaxed', '宽松']] as const).map(([value, label]) => <Button key={value} aria-pressed={appearance.density === value} onClick={() => setAppearance({ ...appearance, density: value })}>{label}</Button>)}</div></div>
          </section>
          <div id="analysis"><AnalysisSettings store={store} onDirty={setSettingsDirty} onSaveReady={registerSave} /></div>
          <div id="storage"><CacheSettings store={store} /></div>
        </div>
      </div>
    </div></main>}

    {page === 'photos' && state.project && <main ref={workspace} className={`workspace ${libraryOpen ? '' : 'no-left'} ${rightOpen ? '' : 'no-right'}`}>
      {!wide && libraryDrawer && <button type="button" className="drawer-scrim" aria-label="关闭照片库面板" onClick={() => setLibraryDrawer(false)} />}
      {libraryOpen && <aside className="sidebar sidebar-left" aria-label="照片库" onKeyDown={event => { if (!wide && event.key === 'Escape') { event.stopPropagation(); setLibraryDrawer(false); } }}>
        <div className="navigator">{current ? <><div className="navigator-frame"><PhotoImage client={store.client} photo={current} kind="preview" /></div><span className="navigator-caption"><span className="filename-line">{current.filename}</span><ScoreChip photo={current} /></span></> : <span className="navigator-empty">未选择照片</span>}</div>
        <div className="sidebar-scroll">
          <Panel id="library" title="照片库"><LibraryNav store={store} onNavigate={() => setLibraryDrawer(false)} /></Panel>
          <Panel id="session" title="本页进度">
            <div className="tally">
              <div className="tally-bar" aria-hidden="true">{tallyOrder.map(key => <span key={key} className={`tally-${key}`} style={{ flexGrow: counts[key] }} />)}</div>
              {tallyOrder.map(key => <div key={key} className={`tally-row tally-${key}`}><span className="tally-swatch" aria-hidden="true" /><span>{decisionNames[key]}</span><span className="tabular">{counts[key]}</span></div>)}
            </div>
          </Panel>
        </div>
        <div className="sidebar-footer"><Button className="ghost block" disabled={running} onClick={() => setScanRoot(state.project!.root)}><Icon name="refresh" />重新扫描文件夹</Button></div>
      </aside>}

      <section className="center" aria-label="照片工作区">
        <div className="toolbar">
          <button type="button" className="tool-button" aria-pressed={libraryOpen} aria-label="照片库面板" title="照片库面板 (B)" onClick={toggleLibrary}><Icon name="library" /></button>
          <div className="view-switch" role="group" aria-label="视图">
            <button type="button" className="view-tab" aria-pressed={view === 'grid'} title="网格 (G)" onClick={() => setView('grid')}><Icon name="grid" /><span>网格</span></button>
            <button type="button" className="view-tab" aria-pressed={view === 'loupe'} disabled={view === 'grid' && !current} title="放大查看 (E)" onClick={() => setView('loupe')}><Icon name="image" /><span>放大</span></button>
            <button type="button" className="view-tab" aria-pressed={view === 'compare'} disabled={running || state.reanalysisRequired || !state.groups.length} title={state.groups.length ? '相似组比较' : '分析后可比较相似照片'} onClick={() => { if (!group && state.groups[0]) attempt(() => store.openGroup(state.groups[0]!.id)); }}><Icon name="compare" /><span>比较</span></button>
          </div>
          <span className="spacer" />
          {view === 'grid' && <button type="button" className="tool-button" aria-pressed={tileInfo} aria-label="显示照片信息" title="显示照片信息" onClick={() => setTileInfo(!tileInfo)}><Icon name="info" /></button>}
          {view === 'loupe' && <button type="button" className="tool-button" aria-pressed={showFaces && hasAnalysis} disabled={!hasAnalysis} aria-label="人脸与眼部标注" title={hasAnalysis ? '人脸与眼部标注 (I)' : '分析后可显示人脸标注'} onClick={() => setShowFaces(!showFaces)}><Icon name="face" /></button>}
          <Button primary className="analyze-button" disabled={running || !!progress?.root_unavailable} onClick={() => attempt(() => store.run('analyze'))}><Icon name="auto-awesome" />{running && progress?.kind === 'analysis' ? '分析中' : '智能分析'}</Button>
          <button type="button" className="tool-button" aria-pressed={rightOpen} aria-label="属性面板" title="属性面板" onClick={() => setRightOpen(!rightOpen)}><Icon name="tune" /></button>
        </div>

        {group && <div className="subbar" aria-label="相似组导航">
          <button type="button" className="sub-button" onClick={() => setView('grid')}><Icon name="back" />返回网格</button>
          <span className="tool-divider" aria-hidden="true" />
          <button type="button" className="sub-icon" aria-label="上一组" title="上一组" disabled={group.loading || groupIndex <= 0} onClick={() => attempt(() => store.moveGroup(-1))}><Icon name="chevron-left" /></button>
          <span className="tabular subbar-title">相似组 {groupIndex + 1}<span className="muted"> / {state.groups.length}</span></span>
          <button type="button" className="sub-icon" aria-label="下一组" title="下一组" disabled={group.loading || groupIndex >= state.groups.length - 1} onClick={() => attempt(() => store.moveGroup(1))}><Icon name="chevron-right" /></button>
          <span className="muted tabular">{group.memberIds.length} 张 · 正在对比 {group.comparisonIds.length} 张</span>
          <span className="spacer" />
          <button type="button" className="sub-button" disabled={group.loading || groupPosition <= 0} onClick={() => store.moveGroupPhoto(-1)}><Icon name="chevron-left" />上一张</button>
          <button type="button" className="sub-button" disabled={group.loading || groupPosition >= group.memberIds.length - 1} onClick={() => store.moveGroupPhoto(1)}>下一张<Icon name="chevron-right" /></button>
        </div>}

        <div className="canvas">
          {group ? <div className={`compare-stage ${fullscreen.active ? 'is-fullscreen' : ''}`} ref={stageRef} aria-busy={group.loading}>
            {group.loading && !group.photos.length ? <div className="canvas-empty" role="status"><span className="activity-ring large" aria-hidden="true" /><p>正在载入相似照片</p></div>
            : !group.comparisonIds.length ? <div className="canvas-empty"><Icon name="compare" className="state-icon" /><p>在下方胶片栏点选要对比的照片</p></div>
            : <div className="compare-grid" style={{ '--columns': Math.min(group.comparisonIds.length, 3) } as CSSProperties}>{group.photos.filter(photo => group.comparisonIds.includes(photo.id)).map(photo => <button key={photo.id} className="compare-cell" disabled={group.loading} aria-label={`查看 ${photo.filename}`} aria-pressed={photo.id === group.focusedId} onClick={() => store.focusGroupPhoto(photo.id)}>
                <span className="compare-frame"><PhotoImage client={store.client} photo={photo} kind="preview" /></span>
                <span className="compare-caption"><span className="filename-line">{photo.filename}</span><ScoreChip photo={photo} /><DecisionBadge decision={photo.decision} />{!!photo.rating && <Stars rating={photo.rating} />}</span>
              </button>)}</div>}
            <div className="stage-hud hud-corner">{group.loading && <span role="status">正在更新</span>}{fullscreenButton}</div>
          </div>
          : view === 'loupe' && current ? <div className={`loupe ${fullscreen.active ? 'is-fullscreen' : ''}`} ref={stageRef}>
            <ZoomViewer key={current.id} client={store.client} photo={current} onZoomChange={onZoomChange} onZoomOutOfFit={() => setView('grid')} apiRef={zoomApi} faces={faces} />
            {zoomReadout.fitted && <>
              <button type="button" className="stage-nav stage-prev" aria-label="上一张" title="上一张 (A / ←)" disabled={reviewIndex <= 0 && offset === 0} onClick={() => attempt(() => store.move(-1))}><Icon name="chevron-left" /></button>
              <button type="button" className="stage-nav stage-next" aria-label="下一张" title="下一张 (D / →)" disabled={reviewIndex >= state.photos.length - 1 && state.photos.length < limit} onClick={() => attempt(() => store.move(1))}><Icon name="chevron-right" /></button>
            </>}
            <div className="stage-hud hud-info">
              <span className="filename-line">{current.filename}</span>
              <DecisionBadge decision={current.decision} />
              {!!current.rating && <Stars rating={current.rating} />}
              {current.color_label && current.color_label !== 'none' && <span className={`color-dot color-${current.color_label}`} title={colorNames[current.color_label]} />}
              <span className="hud-sep" aria-hidden="true" />
              <span className="tabular hud-count">{offset + reviewIndex + 1} / {offset + state.photos.length}</span>
            </div>
            <div className="stage-hud zoom-bar" role="toolbar" aria-label="缩放">
              <button type="button" className="hud-button" aria-label="缩小" title={zoomReadout.fitted ? '缩小到网格 (−)' : '缩小 (−)'} onClick={() => stepZoom(0.8)}><Icon name="zoom-out" /></button>
              <span className="zoom-readout tabular" aria-live="polite">{zoomReadout.fitted ? '适合' : `${zoomReadout.percent}%`}</span>
              <button type="button" className="hud-button" aria-label="放大" title="放大 (+)" onClick={() => zoomApi.current?.zoomBy(1.25)}><Icon name="zoom-in" /></button>
              <span className="hud-divider" aria-hidden="true" />
              <button type="button" className="hud-button" aria-label="适合窗口" title="适合窗口 (Shift+Z)" aria-pressed={zoomReadout.fitted} onClick={() => zoomApi.current?.fit()}><Icon name="fit" /></button>
              <button type="button" className="hud-button hud-text" aria-label="实际像素" title="实际像素 (Shift+Z)" aria-pressed={!zoomReadout.fitted && zoomReadout.percent === 100} onClick={() => zoomApi.current?.actual()}>1:1</button>
              {fullscreen.supported && <span className="hud-divider" aria-hidden="true" />}
              {fullscreenButton}
            </div>
          </div>
          : state.photos.length ? <div className={`grid ${tileInfo ? '' : 'bare'}`} style={{ '--tile': `${tileSize}px` } as CSSProperties}>{state.photos.map((photo, index) => {
            const selected = state.selectedIds.includes(photo.id);
            return <article key={photo.id} data-photo-id={photo.id} className={`cell ${photo.id === state.focusedId ? 'current' : ''} ${selected ? 'selected' : ''} decided-${photo.decision}`} style={{ '--i': Math.min(index, 24) } as CSSProperties}>
              <button className="cell-photo" onClick={event => clickPhoto(photo, index, event)} onDoubleClick={() => openLoupe(photo)} aria-label={`查看 ${photo.filename}，${decisionNames[photo.decision]}，${photo.rating ? `${photo.rating} 星` : '无星标'}，${colorNames[photo.color_label ?? 'none']}`}><PhotoImage client={store.client} photo={photo} /></button>
              <span className="cell-index tabular" aria-hidden="true">{offset + index + 1}</span>
              <label className="cell-check" title="选择"><input type="checkbox" aria-label={`选择 ${photo.filename}`} checked={selected} onChange={event => toggleSelected(photo.id, event.target.checked)} /></label>
              {photo.decision !== 'pending' && <span className={`cell-flag flag-${photo.decision}`} aria-hidden="true" title={decisionNames[photo.decision]}><Icon name={decisionIcons[photo.decision]} /></span>}
              <span className="cell-score-slot" aria-hidden="true"><ScoreChip photo={photo} /></span>
              {tileInfo && <div className="cell-meta" aria-hidden="true">
                <span className="filename-line">{photo.filename}</span>
              <span className="cell-meta-row">{photo.rating ? <Stars rating={photo.rating} /> : <span className="cell-dim">未评级</span>}<span className="cell-tags">{photo.color_label && photo.color_label !== 'none' && <span className={`color-dot color-${photo.color_label}`} />}<span className="cell-format">{photo.format.toUpperCase()}</span></span></span>
              </div>}
            </article>;
          })}</div>
          : <div className="canvas-empty" role="status">{running ? <span className="activity-ring large" aria-hidden="true" /> : <Icon name={activeFilters ? 'filter' : 'library'} className="state-icon" />}<h2>{running ? '正在建立照片列表' : activeFilters ? '没有符合筛选的照片' : '暂无照片'}</h2>{!running && (activeFilters ? <Button onClick={() => attempt(() => store.setFilter({ limit }))}>清除筛选</Button> : <Button primary onClick={() => setScanRoot(state.project!.root)}><Icon name="refresh" />扫描照片文件夹</Button>)}</div>}
        </div>

        <div className={`filmstrip-dock ${stripOpen ? '' : 'collapsed'}`}>
          <div className="filmstrip-head">
            <button type="button" className="strip-toggle" aria-label={stripOpen ? '收起胶片栏' : '展开胶片栏'} title={stripOpen ? '收起胶片栏' : '展开胶片栏'} aria-expanded={stripOpen} onClick={() => setStripOpen(!stripOpen)}><Icon name="expand" className="panel-caret" /></button>
            <MarkTools key={state.project.id} store={store} photo={current} />
            <div className="gallery-tools">
              {!group && <div className="gallery-scale" role="group" aria-label="画册缩放">
                <button type="button" className="tool-button" aria-label="缩小画册" title="缩小画册 (−)" disabled={!current || view === 'grid' && tileSize <= GALLERY_MIN} onClick={() => resizeGallery(view === 'loupe' ? tileSize : Math.round(tileSize / 1.25))}><Icon name="grid" /></button>
                <input type="range" min={GALLERY_MIN} max={GALLERY_LOUPE} step={1} value={view === 'loupe' ? GALLERY_LOUPE : tileSize} aria-label="缩略图大小" aria-valuetext={view === 'loupe' ? '单图' : `缩略图 ${tileSize} 像素`} title="画册缩放 · 最右端进入单图 · Ctrl + 滚轮" disabled={!current} onChange={event => resizeGallery(Number(event.target.value))} />
                <button type="button" className="tool-button" aria-label="放大到单图" title="放大到单图 (E)" disabled={!current} onClick={() => setView('loupe')}><Icon name="image" /></button>
              </div>}
              <div className="filter-anchor"><button type="button" className={`tool-pill ${activeFilters ? 'active' : ''}`} aria-label="筛选照片" title="筛选照片" aria-expanded={filterOpen} onClick={() => setFilterOpen(true)}><Icon name="filter" /><span className="filter-label">筛选</span>{activeFilters > 0 && <span className="pill-count tabular">{activeFilters}</span>}</button>
                {filterOpen && <PhotoFilters filter={state.filter} onChange={filter => attempt(() => store.setFilter(filter))} onClose={() => setFilterOpen(false)} />}
              </div>
            </div>
            {activeFilters > 0 && <div className="chips">{Object.entries(filterLabels).map(([key, label]) => {
              const typed = key as keyof typeof state.filter;
              const value = state.filter[typed];
              return value === undefined ? null : <button type="button" key={key} className="filter-chip" aria-label={`清除${label}筛选`} onClick={() => attempt(() => { const next = { ...state.filter, offset: 0 }; delete next[typed]; return store.setFilter(next); })}><span className="muted">{label}</span>{filterValue(typed, value)}<Icon name="close" /></button>;
            })}</div>}
          </div>
          {stripOpen && (group ? <div className="filmstrip" aria-label="相似组照片">{group.photos.map(photo => <button key={photo.id} data-photo-id={photo.id} className={`film ${photo.id === group.focusedId ? 'current' : ''}`} disabled={group.loading} aria-label={`${group.comparisonIds.includes(photo.id) ? '移出对比' : '加入对比'}：${photo.filename}`} aria-pressed={group.comparisonIds.includes(photo.id)} onClick={() => { const ids = group.comparisonIds.includes(photo.id) ? group.comparisonIds.filter(id => id !== photo.id) : [...group.comparisonIds, photo.id]; store.setComparisonCandidates(ids); }}>
              <PhotoImage client={store.client} photo={photo} />{group.comparisonIds.includes(photo.id) && <span className="film-check" aria-hidden="true"><Icon name="done" /></span>}{photo.decision !== 'pending' && <span className={`film-flag flag-${photo.decision}`} aria-hidden="true" />}
            </button>)}</div>
            : <div className="filmstrip" aria-label="照片胶片栏">{state.photos.map((photo, index) => <button key={photo.id} data-photo-id={photo.id} className={`film ${photo.id === state.focusedId ? 'current' : ''} ${state.selectedIds.includes(photo.id) ? 'selected' : ''}`} aria-label={photo.filename} aria-pressed={photo.id === state.focusedId} onClick={event => clickPhoto(photo, index, event)} onDoubleClick={() => openLoupe(photo)}><PhotoImage client={store.client} photo={photo} />{photo.decision !== 'pending' && <span className={`film-flag flag-${photo.decision}`} aria-hidden="true" />}{!!photo.rating && <span className="film-rating tabular" aria-hidden="true">{photo.rating}<Icon name="star" /></span>}</button>)}</div>)}
        </div>

        <footer className="statusbar">
          <span className="tabular">{view === 'grid' ? <>本页 <b>{state.photos.length}</b> 张</> : current ? <><b>{current.width}</b> × <b>{current.height}</b> · {current.format.toUpperCase()}</> : '—'}</span>
          {bulk && <span className="tabular selection-count">已选 {state.selectedIds.length} 张</span>}
          {view === 'grid' && state.photos.length > 0 && <button type="button" className="status-link" onClick={() => store.select(state.selectedIds.length === state.photos.length ? [] : state.photos.map(photo => photo.id))}>{state.selectedIds.length === state.photos.length ? '取消全选' : '全选本页'}</button>}
          {view === 'grid' && <button type="button" className="status-link" onClick={() => setStripOpen(!stripOpen)}>{stripOpen ? '隐藏胶片栏' : '显示胶片栏'}</button>}
          <span className="spacer" />
          <span className="status-hint"><kbd>W</kbd>保留<kbd>S</kbd>移除<kbd>P</kbd>标记<kbd>0–5</kbd>星级<kbd>?</kbd>全部</span>
          <span className="tool-divider" aria-hidden="true" />
          <button type="button" className="sub-icon" aria-label="上一页" title="上一页" disabled={!!group || offset === 0} onClick={() => attempt(() => store.setFilter({ ...state.filter, offset: Math.max(0, offset - limit) }))}><Icon name="chevron-left" /></button>
          <span className="tabular page-number">第 {Math.floor(offset / limit) + 1} 页</span>
          <button type="button" className="sub-icon" aria-label="下一页" title="下一页" disabled={!!group || state.photos.length < limit} onClick={() => attempt(() => store.setFilter({ ...state.filter, offset: offset + limit }))}><Icon name="chevron-right" /></button>
        </footer>
      </section>

      {rightOpen && <aside className="sidebar sidebar-right" aria-label="属性">
        <Histogram client={store.client} photo={current} />
        {current && <div className="exif-strip tabular"><span>{current.width}×{current.height}</span><span>{(current.width * current.height / 1e6).toFixed(1)} MP</span><span>{current.format.toUpperCase()}</span><span>{formatBytes(current.size_bytes)}</span></div>}
        <div className="sidebar-scroll">
          {current && !bulk && <Panel id="analysis" title="智能评估" aside={<ScoreChip photo={current} />}><AnalysisPanel photo={current} /></Panel>}
          {current && !bulk && <Panel id="metadata" title="元数据"><Metadata photo={current} /></Panel>}
          <Panel id="files" title="文件操作" defaultOpen={false}><FileWorkflows key={state.project.id} store={store} /></Panel>
        </div>
      </aside>}
    </main>}

    <div ref={taskPanel} id="task-center" className={`task-center${tasksOpen ? ' open' : ''}`} role={tasksOpen ? 'region' : undefined} aria-label={tasksOpen ? '任务详情' : undefined}>
      {tasksOpen && <div className="task-center-heading"><h2>任务</h2><button type="button" className="title-button" aria-label="关闭任务详情" title="关闭任务详情" onClick={() => { setTasksOpen(false); taskTrigger.current?.focus(); }}><Icon name="close" /></button></div>}
      <TaskBar store={store} expanded={tasksOpen} onSettings={() => { setTasksOpen(false); navigate('settings'); }} />
      <ModelTaskBar store={store} expanded={tasksOpen} onSettings={() => { setTasksOpen(false); navigate('settings'); }} />
    </div>
    {shortcutsOpen && <Dialog title="键盘快捷键" onClose={() => setShortcutsOpen(false)}><ShortcutSheet /><div className="dialog-actions"><Button primary onClick={() => setShortcutsOpen(false)}>知道了</Button></div></Dialog>}
    {scanRoot && <Dialog title="扫描照片文件夹" onClose={() => { if (!busy) setScanRoot(null); }}><div className="stack"><p className="path-box filename">{scanRoot}</p><dl><dt>扫描范围</dt><dd>当前文件夹及子文件夹</dd><dt>文件处理</dt><dd>保留原位置</dd></dl></div>{(error || state.error) && <p role="alert" className="inline-error">{error || state.error}</p>}<div className="dialog-actions"><Button disabled={busy} onClick={() => setScanRoot(null)}>取消</Button><Button primary disabled={busy} onClick={() => attempt(startScan)}>{busy ? '正在打开' : '开始扫描'}</Button></div></Dialog>}
    {leaveTarget && <Dialog title="离开设置？" onClose={() => { if (!savingLeave) setLeaveTarget(null); }}><p>分析参数尚未保存。</p><div className="dialog-actions"><Button disabled={savingLeave} onClick={() => setLeaveTarget(null)}>继续编辑</Button><Button disabled={savingLeave} onClick={() => { setPage(leaveTarget); setSettingsDirty(false); setLeaveTarget(null); }}>放弃更改</Button><Button primary disabled={savingLeave} onClick={() => { setSavingLeave(true); setError(null); void (async () => { try { if (await saveSettings.current?.()) { setPage(leaveTarget); setSettingsDirty(false); setLeaveTarget(null); } else setError('保存未完成，请继续编辑并检查参数'); } catch (reason) { setError(String(reason)); } finally { setSavingLeave(false); } })(); }}>{savingLeave ? '正在保存' : '保存后返回'}</Button></div>{error && <p role="alert" className="inline-error">{error}</p>}</Dialog>}
    {closeRequested && <Dialog title="关闭伊人？" onClose={() => setCloseRequested(false)}><p>未保存的分析参数将被放弃。</p><div className="dialog-actions"><Button onClick={() => setCloseRequested(false)}>继续编辑</Button><Button primary onClick={() => attempt(async () => { await window.__TAURI__?.core.invoke('confirm_close'); })}>放弃更改并关闭</Button></div>{error && <p role="alert" className="inline-error">{error}</p>}</Dialog>}
  </div>;
}

function AnalysisPanel({ photo }: { photo: Photo }) {
  const analysis = readPhotoAnalysis(photo);
  const verdict = analysis.verdict ? verdictNames[analysis.verdict] : analysis.status === 'stale' ? '需要重新分析' : '尚未分析';
  if (!analysis.current) return <div className="analysis-empty"><Icon name="auto-awesome" /><div><p>{verdict}</p><p className="muted">运行“智能分析”后，这里会显示人脸、眼部与清晰度评估。</p></div></div>;
  const faces = analysis.current.faces;
  const closed = faces.filter(face => face.left_eye?.state === 'closed' || face.right_eye?.state === 'closed').length;
  const components = analysis.scoreBreakdown?.components.filter(item => item.score !== null && item.score !== undefined) ?? [];
  const score = analysis.score === null ? null : Math.round(analysis.score);
  return <div className="analysis">
    <div className="analysis-head">
      <div className={`score-ring verdict-${analysis.verdict ?? 'none'}`} style={{ '--p': score ?? 0 } as CSSProperties}>
        <svg viewBox="0 0 36 36" aria-hidden="true"><circle className="ring-track" cx="18" cy="18" r="15.5" /><circle className="ring-value" cx="18" cy="18" r="15.5" pathLength="100" /></svg>
        <span className="ring-score tabular">{score ?? '—'}</span>
      </div>
      <div className="analysis-summary">
        <p className={`analysis-verdict verdict-${analysis.verdict ?? 'none'}`}>{verdict}</p>
        <p className="muted analysis-faces"><Icon name="face" />{faces.length ? `${faces.length} 张人脸${closed ? ` · ${closed} 人闭眼` : ''}` : '未检测到人脸'}</p>
      </div>
    </div>
    {components.length > 0 && <ul className="meters">{components.map(item => {
      const value = Math.max(0, Math.min(100, item.score ?? 0));
      return <li key={item.id}><span className="meter-label">{componentNames[item.id] ?? item.id}</span><span className="meter" aria-hidden="true"><span style={{ width: `${value}%` }} className={value >= 70 ? 'good' : value >= 45 ? 'warn' : 'bad'} /></span><span className="tabular meter-value">{Math.round(value)}</span></li>;
    })}</ul>}
    {analysis.current.warnings.length > 0 && <details className="analysis-warnings"><summary>{analysis.current.warnings.length} 条分析提示</summary>{analysis.current.warnings.map((message, index) => <p key={index} className="muted">{message}</p>)}</details>}
  </div>;
}

function Metadata({ photo }: { photo: Photo }) {
  const taken = formatTaken(photo.taken_at);
  const rows: [string, ReactNode][] = [
    ['文件名', <span className="filename">{photo.filename}</span>],
    ...(taken ? [['拍摄时间', <span className="tabular">{taken}</span>] as [string, ReactNode]] : []),
    ['尺寸', <span className="tabular">{photo.width} × {photo.height}</span>],
    ['像素', <span className="tabular">{(photo.width * photo.height / 1e6).toFixed(1)} MP</span>],
    ['格式', photo.format.toUpperCase()],
    ['大小', <span className="tabular">{formatBytes(photo.size_bytes)}</span>],
  ];
  return <div className="metadata">
    <dl className="meta-table">{rows.map(([label, value]) => <div key={label} className="meta-row"><dt>{label}</dt><dd>{value}</dd></div>)}</dl>
    <details><summary>文件位置</summary><p className="filename path-text">{photo.path}</p></details>
  </div>;
}

function ShortcutSheet() {
  const groups: [string, [string[], string][]][] = [
    ['标记', [[['W', '↑'], '保留并前进'], [['S', '↓'], '标记移除并前进'], [['P'], '标记'], [['1', '–', '5'], '设置星级'], [['0'], '清除星级'], [['Z'], '撤销']]],
    ['浏览', [[['A', '←'], '上一张'], [['D', '→'], '下一张'], [['G'], '网格视图'], [['E'], '放大查看'], [['空格'], '网格 / 放大切换'], [['Ctrl', 'A'], '全选本页']]],
    ['查看', [[['+', '−'], '放大 / 缩小'], [['Shift', 'Z'], '适合 ↔ 1:1'], [['滚轮'], '以指针为中心缩放'], [['双击'], '放大 / 复位'], [['F'], '全屏'], [['I'], '人脸标注'], [['B'], '隐藏 / 显示面板']]],
  ];
  return <div className="shortcut-sheet">{groups.map(([title, items]) => <section key={title}><h3>{title}</h3><dl>{items.map(([keys, label]) => <div key={label} className="shortcut-row"><dt>{keys.map(key => key === '–' ? <span key={key} className="key-sep">–</span> : <kbd key={key}>{key}</kbd>)}</dt><dd>{label}</dd></div>)}</dl></section>)}</div>;
}
