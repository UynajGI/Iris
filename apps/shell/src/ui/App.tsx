import { useCallback, useEffect, useRef, useState, type CSSProperties } from 'react';
import type { IrisStore } from '../store.js';
import type { CommandSystem } from '../commands.js';
import type { Action, Photo, Project } from '../types.js';
import { useIris } from '../react.js';
import { readPhotoAnalysis } from '../analysis-readout.js';
import { selectProjectFolder } from '../native-dialog.js';
import { Button, Dialog, PhotoImage, Popover } from './components.js';
import { loadAppearance, saveAppearance, type Appearance } from './preferences.js';
import { AnalysisSettings } from './AnalysisSettings.js';
import { Icon } from './Icon.js';
import { FileWorkflows } from './FileWorkflows.js';
import { CacheSettings } from './CacheSettings.js';
import { MarkTools, colorNames } from './MarkTools.js';
import { PhotoFilters, filterLabels, filterValue } from './PhotoFilters.js';
import { TaskBar } from './TaskBar.js';
import { ModelTaskBar } from './ModelTaskBar.js';
import { LibraryNav } from './LibraryNav.js';
import { ZoomViewer, type ZoomApi } from './ZoomViewer.js';
import { useFullscreen } from './fullscreen.js';
import './app.css';

const decisionNames: Record<Action, string> = { pending: '未标记', keep: '保留', flag: '已标记', reject: '标记移除' };
const verdictNames = { recommend: '建议保留', review: '需要复核', reject_suggest: '建议移除' };

function Stars({ rating }: { rating: number }) {
  return <span className="tile-stars" role="img" aria-label={`${rating} 星`}>{Array.from({ length: rating }, (_, index) => <Icon key={index} name="star" />)}</span>;
}

function PhotoMarks({ photo }: { photo: Photo }) {
  return <>
    {photo.decision !== 'pending' && <span className={`decision-label decision-${photo.decision}`}>{decisionNames[photo.decision]}</span>}
    {!!photo.rating && <Stars rating={photo.rating} />}
    {photo.color_label && photo.color_label !== 'none' && <span className="tile-color"><span className={`color-dot color-${photo.color_label}`} aria-hidden="true" />{colorNames[photo.color_label]}</span>}
  </>;
}

export function App({ store, commands }: { store: IrisStore; commands: CommandSystem }) {
  const state = useIris(store);
  const [page, setPage] = useState<'projects' | 'photos' | 'settings'>(() => state.project ? 'photos' : 'projects');
  const [appearance, setAppearance] = useState<Appearance>(loadAppearance);
  const [error, setError] = useState<string | null>(null);
  const [scanRoot, setScanRoot] = useState<string | null>(null);
  const [filterOpen, setFilterOpen] = useState(false);
  const [directoryOpen, setDirectoryOpen] = useState(false);
  const [inaccessible, setInaccessible] = useState<{ project: Project; reason: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [revealTasks, setRevealTasks] = useState(0);
  const [settingsDirty, setSettingsDirty] = useState(false);
  const [closeRequested, setCloseRequested] = useState(false);
  const [leaveTarget, setLeaveTarget] = useState<'projects' | 'photos' | null>(null);
  const [savingLeave, setSavingLeave] = useState(false);
  const saveSettings = useRef<(() => Promise<boolean>) | null>(null);
  const registerSave = useCallback((save: (() => Promise<boolean>) | null) => { saveSettings.current = save; }, []);
  const photoArea = useRef<HTMLElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  const zoomApi = useRef<ZoomApi | null>(null);
  const [zoomReadout, setZoomReadout] = useState({ percent: 100, fitted: true });
  const onZoomChange = useCallback((percent: number, fitted: boolean) => setZoomReadout({ percent, fitted }), []);
  const fullscreen = useFullscreen(stageRef);
  const [tileSize, setTileSize] = useState(() => { const saved = Number(localStorage.getItem('iris:tile-size:v1')); return saved >= 140 && saved <= 360 ? saved : 200; });
  useEffect(() => { try { localStorage.setItem('iris:tile-size:v1', String(tileSize)); } catch { /* Session size stays usable. */ } }, [tileSize]);
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
    photoArea.current?.querySelector(`[data-photo-id="${current.id}"]`)?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }, [page, current?.id, state.reviewOpen, group?.groupId]);
  useEffect(() => {
    if (page !== 'photos' || scanRoot || filterOpen || directoryOpen) return;
    return commands.attach(window, reason => setError(String(reason)));
  }, [commands, page, scanRoot, filterOpen, directoryOpen]);
  useEffect(() => {
    if (page !== 'photos' || scanRoot || filterOpen || directoryOpen) return;
    const viewing = state.reviewOpen || !!state.groupSession;
    const handler = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (event.defaultPrevented || event.ctrlKey || event.metaKey || event.altKey || /^(INPUT|TEXTAREA|SELECT)$/.test(target?.tagName ?? '')) return;
      if (event.key === 'f' || event.key === 'F') { if (!viewing) return; event.preventDefault(); void fullscreen.toggle().catch(reason => setError(String(reason))); return; }
      if (!state.reviewOpen || state.groupSession) return;
      const api = zoomApi.current;
      if (!api) return;
      if (event.key === '=' || event.key === '+') { event.preventDefault(); api.zoomBy(1.25); }
      else if (event.key === '-' || event.key === '_') { event.preventDefault(); api.zoomBy(0.8); }
      else if (event.key === '0') { event.preventDefault(); api.fit(); }
      else if (event.key === '1') { event.preventDefault(); api.actual(); }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [page, scanRoot, filterOpen, directoryOpen, state.reviewOpen, state.groupSession, fullscreen]);
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
  const running = progress && ['running', 'paused', 'cancelling'].includes(progress.state);
  const hasFilters = Object.keys(filterLabels).some(key => state.filter[key as keyof typeof state.filter] !== undefined);
  const limit = state.filter.limit ?? 200;
  const offset = state.filter.offset ?? 0;
  const reviewIndex = current ? state.photos.findIndex(photo => photo.id === current.id) : -1;
  const groupPosition = group ? group.memberIds.indexOf(group.focusedId!) : -1;
  const view = group ? 'compare' : state.reviewOpen ? 'review' : 'grid';
  const setView = (target: 'grid' | 'review') => {
    if (group) store.closeGroup();
    if ((target === 'review') !== state.reviewOpen) store.toggleReview();
  };
  const toggleSelected = (id: number, checked: boolean) => store.select(checked ? [...state.selectedIds, id] : state.selectedIds.filter(item => item !== id));

  return <div className="app">
    <header className="app-header">
      <span className="brand">伊人</span>
      <nav className="segmented app-navigation" aria-label="主导航">
        <Button aria-current={page === 'projects' ? 'page' : undefined} onClick={() => navigate('projects')}><Icon name="folder" />项目</Button>
        {state.project && <Button aria-current={page === 'photos' ? 'page' : undefined} onClick={() => navigate('photos')}><Icon name="grid" />选片</Button>}
      </nav>
      <span className="project-name" title={state.project?.root}>{state.project && <><strong>{state.project.name}</strong><span className="muted">{state.project.root}</span></>}</span>
      <span className={`connection connection-${state.connection}`} role="status" title={state.connection === 'disconnected' ? '服务连接已断开' : '服务已连接'}><span className="connection-dot" aria-hidden="true" /><span className="visually-hidden">{state.connection === 'disconnected' ? '服务连接已断开' : '服务已连接'}</span></span>
      <Button className="quiet task-toggle" onClick={() => setRevealTasks(value => value + 1)}>{running ? <span className="activity-dot" aria-hidden="true" /> : <Icon name="info" />}任务</Button>
      <Button className="quiet" aria-current={page === 'settings' ? 'page' : undefined} onClick={() => navigate('settings')}><Icon name="settings" />设置</Button>
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

    {page === 'projects' && !inaccessible && <main className="page projects-page"><div className="page-content">
      <section className="project-intro">
        <h1 className="brand display">伊人</h1>
        <p className="muted">浏览、比较，留下你的选择。</p>
        <Button primary className="large" disabled={busy} onClick={() => attempt(pickFolder)}><Icon name="folder" />打开照片文件夹</Button>
      </section>
      <div className="section-heading"><h2>最近项目</h2><span className="muted tabular">{state.projects.length ? `${state.projects.length} 个项目` : ''}</span></div>
      {state.projects.length === 0 ? <div className="project-empty"><Icon name="library" /><p>暂无项目</p><p className="muted">打开的照片文件夹会显示在这里。</p></div>
        : <ul className="project-list">{state.projects.map((project, index) => <li key={project.id} className="project-row" style={{ '--index': index } as CSSProperties}>
          <button className="project-open" title={project.root} onClick={() => attempt(() => openRecent(project))}>
            <span className="project-folder" aria-hidden="true"><Icon name="folder" /></span>
            <span className="project-text"><strong>{project.name}</strong><span className="muted">{project.root}</span></span>
            {state.project?.id === project.id && <span className="current-badge">当前</span>}
            <Icon name="chevron-right" className="project-chevron" />
          </button>
          <Button className="quiet project-remove" onClick={() => attempt(() => store.hideRecentProject(project.id))}>移出列表</Button>
        </li>)}</ul>}
    </div></main>}

    {page === 'settings' && <main className="page settings-page"><div className="page-content">
      <header className="page-heading">
        {state.project && <Button className="quiet icon-only" aria-label="返回选片" title="返回选片" onClick={() => navigate('photos')}><Icon name="back" /></Button>}
        <div><h1>设置</h1><p className="muted">{state.project ? state.project.name : '应用偏好'}</p></div>
      </header>
      <div className="settings-layout">
        <nav className="settings-nav" aria-label="设置分类"><a href="#appearance">外观</a><a href="#analysis">模型与分析</a>{state.project && <a href="#storage">存储与缓存</a>}</nav>
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

    {page === 'photos' && state.project && <main className={`workspace ${view !== 'grid' ? 'review' : ''}`}>
      {view === 'grid' && <nav className="library" aria-label="照片分类"><LibraryNav store={store} /></nav>}
      <section ref={photoArea} className="photo-area" aria-label="照片工作区">
        <div className="photo-toolbar">
          <div className={`filter-anchor ${view !== 'grid' ? '' : 'narrow-only'}`}><Button className="quiet collapsible" title="照片目录" aria-expanded={directoryOpen} onClick={() => setDirectoryOpen(true)}><Icon name="library" />目录</Button>{directoryOpen && <Popover title="照片目录" onClose={() => setDirectoryOpen(false)}><LibraryNav store={store} onNavigate={() => setDirectoryOpen(false)} /></Popover>}</div>
          <div className="segmented view-switch" role="group" aria-label="视图">
            <Button aria-pressed={view === 'grid'} onClick={() => setView('grid')}><Icon name="grid" />总览</Button>
            <Button aria-pressed={view === 'review'} disabled={view === 'grid' && !current} onClick={() => setView('review')}><Icon name="image" />单张</Button>
            {group && <span className="segment-static" aria-current="true"><Icon name="compare" />比较</span>}
          </div>
          <div className="filter-anchor"><Button className={hasFilters ? 'filter-active' : 'quiet'} aria-expanded={filterOpen} onClick={() => setFilterOpen(true)}><Icon name="filter" />筛选</Button>
            {filterOpen && <PhotoFilters filter={state.filter} onChange={filter => attempt(() => store.setFilter(filter))} onClose={() => setFilterOpen(false)} />}
          </div>
          {Object.entries(filterLabels).map(([key, label]) => {
            const typed = key as keyof typeof state.filter;
            const value = state.filter[typed];
            return value === undefined ? null : <button type="button" key={key} className="filter-chip" aria-label={`清除${label}筛选`} onClick={() => attempt(() => { const next = { ...state.filter, offset: 0 }; delete next[typed]; return store.setFilter(next); })}><span className="muted">{label}</span>{filterValue(typed, value)}<Icon name="close" /></button>;
          })}
          <span className="spacer" />
          <Button className="quiet collapsible" title="重新扫描" disabled={!!running} onClick={() => setScanRoot(state.project!.root)}><Icon name="refresh" />重新扫描</Button>
          <Button primary disabled={!!running || !!progress?.root_unavailable} onClick={() => attempt(() => store.run('analyze'))}><Icon name="play" />开始分析</Button>
        </div>

        {group && <div className="sub-toolbar" aria-label="相似组导航">
          <Button className="quiet" onClick={() => store.closeGroup()}><Icon name="back" />返回总览</Button>
          <span className="toolbar-divider" aria-hidden="true" />
          <Button className="quiet icon-only" aria-label="上一组" title="上一组" disabled={group.loading || groupIndex <= 0} onClick={() => attempt(() => store.moveGroup(-1))}><Icon name="chevron-left" /></Button>
          <span className="tabular">第 {groupIndex + 1} / {state.groups.length} 组</span>
          <Button className="quiet icon-only" aria-label="下一组" title="下一组" disabled={group.loading || groupIndex >= state.groups.length - 1} onClick={() => attempt(() => store.moveGroup(1))}><Icon name="chevron-right" /></Button>
          <span className="muted tabular">{group.memberIds.length} 张 · 对比 {group.comparisonIds.length} 张</span>
          <span className="spacer" />
          <Button className="quiet" disabled={group.loading || groupPosition <= 0} onClick={() => store.moveGroupPhoto(-1)}><Icon name="chevron-left" />上一张</Button>
          <Button className="quiet" disabled={group.loading || groupPosition >= group.memberIds.length - 1} onClick={() => store.moveGroupPhoto(1)}>下一张<Icon name="chevron-right" /></Button>
        </div>}

        {group ? <>
          {group.loading ? <div className="empty photo-empty" role="status"><span className="activity-dot" aria-hidden="true" /><p>正在载入相似照片</p></div>
            : !group.comparisonIds.length ? <div className="empty photo-empty"><Icon name="compare" /><p>从下方选择要对比的照片</p></div>
            : <div className={`compare-grid ${fullscreen.active ? 'is-fullscreen' : ''}`} ref={stageRef} style={{ '--columns': Math.min(group.comparisonIds.length, 3) } as CSSProperties}>{group.photos.filter(photo => group.comparisonIds.includes(photo.id)).map(photo => <button key={photo.id} className="compare-image" aria-label={`查看 ${photo.filename}`} aria-pressed={photo.id === group.focusedId} onClick={() => store.focusGroupPhoto(photo.id)}>
              <span className="compare-frame"><PhotoImage client={store.client} photo={photo} kind="preview" /></span>
              <span className="compare-caption"><span className="filename-line">{photo.filename}</span><PhotoMarks photo={photo} /></span>
            </button>)}{fullscreen.supported && <div className="stage-hud zoom-bar compare-hud"><button type="button" className="hud-button" aria-label={fullscreen.active ? '退出全屏' : '全屏'} title={fullscreen.active ? '退出全屏 (F / Esc)' : '全屏 (F)'} onClick={() => void fullscreen.toggle().catch(reason => setError(String(reason)))}><Icon name={fullscreen.active ? 'fullscreen-exit' : 'fullscreen'} /></button></div>}</div>}
          <div className="filmstrip" aria-label="相似组照片">{group.photos.map(photo => <button key={photo.id} data-photo-id={photo.id} className={`film-item ${photo.id === group.focusedId ? 'current' : ''}`} aria-label={`${group.comparisonIds.includes(photo.id) ? '移出对比' : '加入对比'}：${photo.filename}`} aria-pressed={group.comparisonIds.includes(photo.id)} onClick={() => { const ids = group.comparisonIds.includes(photo.id) ? group.comparisonIds.filter(id => id !== photo.id) : [...group.comparisonIds, photo.id]; store.setComparisonCandidates(ids); }}>
            <PhotoImage client={store.client} photo={photo} />{group.comparisonIds.includes(photo.id) && <span className="film-check" aria-hidden="true"><Icon name="done" /></span>}{photo.decision !== 'pending' && <span className={`film-decision decision-${photo.decision}`} aria-hidden="true" />}
          </button>)}</div>
        </>
        : view === 'review' && current ? <>
          <div className={`review-stage ${fullscreen.active ? 'is-fullscreen' : ''}`} ref={stageRef}>
            <ZoomViewer key={current.id} client={store.client} photo={current} onZoomChange={onZoomChange} apiRef={zoomApi} />
            {zoomReadout.fitted && <>
              <button type="button" className="stage-nav stage-prev" aria-label="上一张" title="上一张 (A)" disabled={reviewIndex <= 0 && offset === 0} onClick={() => attempt(() => store.move(-1))}><Icon name="chevron-left" /></button>
              <button type="button" className="stage-nav stage-next" aria-label="下一张" title="下一张 (D)" disabled={reviewIndex >= state.photos.length - 1 && state.photos.length < limit} onClick={() => attempt(() => store.move(1))}><Icon name="chevron-right" /></button>
            </>}
            {fullscreen.active && <div className="stage-hud stage-hud-top"><span className="filename-line">{current.filename}</span><span className="hud-marks"><PhotoMarks photo={current} /></span><span className="spacer" /><span className="tabular">{offset + reviewIndex + 1} / {offset + state.photos.length}</span></div>}
            <div className="stage-hud zoom-bar" role="toolbar" aria-label="缩放">
              <button type="button" className="hud-button" aria-label="缩小" title="缩小 (−)" onClick={() => zoomApi.current?.zoomBy(0.8)}><Icon name="zoom-out" /></button>
              <span className="zoom-readout tabular" aria-live="polite">{zoomReadout.fitted ? '适合' : `${zoomReadout.percent}%`}</span>
              <button type="button" className="hud-button" aria-label="放大" title="放大 (+)" onClick={() => zoomApi.current?.zoomBy(1.25)}><Icon name="zoom-in" /></button>
              <span className="hud-divider" aria-hidden="true" />
              <button type="button" className="hud-button" aria-label="适合窗口" title="适合窗口 (0)" aria-pressed={zoomReadout.fitted} onClick={() => zoomApi.current?.fit()}><Icon name="fit" /></button>
              <button type="button" className="hud-button hud-text" aria-label="实际像素" title="实际像素 (1)" aria-pressed={!zoomReadout.fitted && zoomReadout.percent === 100} onClick={() => zoomApi.current?.actual()}>1:1</button>
              {fullscreen.supported && <><span className="hud-divider" aria-hidden="true" /><button type="button" className="hud-button" aria-label={fullscreen.active ? '退出全屏' : '全屏'} title={fullscreen.active ? '退出全屏 (F / Esc)' : '全屏 (F)'} onClick={() => void fullscreen.toggle().catch(reason => setError(String(reason)))}><Icon name={fullscreen.active ? 'fullscreen-exit' : 'fullscreen'} /></button></>}
            </div>
          </div>
          <div className="review-caption"><span className="filename-line" title={current.filename}>{current.filename}</span><span className="review-marks"><PhotoMarks photo={current} /></span><span className="spacer" /><span className="muted tabular">{offset + reviewIndex + 1} / 本页 {state.photos.length}</span></div>
          <div className="filmstrip" aria-label="照片胶片栏">{state.photos.map(photo => <button key={photo.id} data-photo-id={photo.id} className={`film-item ${photo.id === current.id ? 'current' : ''}`} aria-label={photo.filename} aria-pressed={photo.id === current.id} onClick={() => store.focus(photo.id)}><PhotoImage client={store.client} photo={photo} />{photo.decision !== 'pending' && <span className={`film-decision decision-${photo.decision}`} aria-hidden="true" />}</button>)}</div>
        </>
        : state.photos.length ? <div className="photo-grid" style={{ '--tile': `${tileSize}px` } as CSSProperties}>{state.photos.map(photo => {
          const selected = state.selectedIds.includes(photo.id);
          return <article key={photo.id} data-photo-id={photo.id} className={`photo-tile ${photo.id === state.focusedId ? 'current' : ''} ${selected ? 'selected' : ''} decided-${photo.decision}`}>
            <button className="photo-button" onClick={event => { if (event.metaKey || event.ctrlKey) toggleSelected(photo.id, !selected); else store.focus(photo.id); }} onDoubleClick={() => { store.focus(photo.id); if (!state.reviewOpen) store.toggleReview(); }} aria-label={`查看 ${photo.filename}，${decisionNames[photo.decision]}，${photo.rating ? `${photo.rating} 星` : '无星标'}，${colorNames[photo.color_label ?? 'none']}`}><PhotoImage client={store.client} photo={photo} /></button>
            <label className="photo-check"><input type="checkbox" aria-label={`选择 ${photo.filename}`} checked={selected} onChange={event => toggleSelected(photo.id, event.target.checked)} /></label>
            {photo.decision !== 'pending' && <span className={`tile-decision decision-${photo.decision}`} aria-hidden="true"><Icon name={photo.decision === 'keep' ? 'done' : photo.decision === 'flag' ? 'flag' : 'close'} />{decisionNames[photo.decision]}</span>}
            {!!photo.rating && <span className="tile-rating" aria-hidden="true"><Stars rating={photo.rating} /></span>}
            <div className="photo-caption" aria-hidden="true">{photo.color_label && photo.color_label !== 'none' && <span className={`color-dot color-${photo.color_label}`} title={colorNames[photo.color_label]} />}<span className="filename-line" title={photo.filename}>{photo.filename}</span><span className="tile-format">{photo.format.toUpperCase()}</span></div>
          </article>;
        })}</div>
        : <div className="empty photo-empty" role="status">{running ? <span className="activity-dot large" aria-hidden="true" /> : <Icon name={hasFilters ? 'filter' : 'library'} className="state-icon" />}<h2>{running ? '正在建立照片列表' : hasFilters ? '没有符合筛选的照片' : '暂无照片'}</h2>{!running && (hasFilters ? <Button onClick={() => attempt(() => store.setFilter({ limit }))}>清除筛选</Button> : <Button primary onClick={() => setScanRoot(state.project!.root)}><Icon name="refresh" />扫描照片文件夹</Button>)}</div>}

        {view === 'grid' && <footer className="workspace-footer"><span className="muted tabular">本页 {state.photos.length} 张{state.selectedIds.length > 0 && <> · <span className="selection-count">已选 {state.selectedIds.length} 张</span></>}</span>
          {state.photos.length > 0 && <Button className="quiet small" onClick={() => store.select(state.selectedIds.length === state.photos.length ? [] : state.photos.map(photo => photo.id))}>{state.selectedIds.length === state.photos.length ? '取消全选' : '全选本页'}</Button>}
          <span className="spacer" />
          <label className="tile-size" title="缩略图大小"><Icon name="grid" /><input type="range" min={140} max={360} step={20} value={tileSize} aria-label="缩略图大小" onChange={event => setTileSize(Number(event.target.value))} /><Icon name="image" /></label>
          <span className="toolbar-divider" aria-hidden="true" />
          <Button className="quiet icon-only" aria-label="上一页" title="上一页" disabled={offset === 0} onClick={() => attempt(() => store.setFilter({ ...state.filter, offset: Math.max(0, offset - limit) }))}><Icon name="chevron-left" /></Button>
          <span className="page-number tabular" aria-label="当前页">第 {Math.floor(offset / limit) + 1} 页</span>
          <Button className="quiet icon-only" aria-label="下一页" title="下一页" disabled={state.photos.length < limit} onClick={() => attempt(() => store.setFilter({ ...state.filter, offset: offset + limit }))}><Icon name="chevron-right" /></Button>
        </footer>}
      </section>
      <aside className="inspector" aria-label="标记与详情">
        <div className="inspector-tools"><MarkTools key={state.project.id} store={store} photo={current} /></div>
        <div className="inspector-secondary">
          {current && !(state.selectedIds.length && !group) && <PhotoDetails photo={current} />}
          <FileWorkflows key={state.project.id} store={store} />
        </div>
      </aside>
    </main>}

    <TaskBar store={store} reveal={revealTasks} onSettings={() => navigate('settings')} />
    <ModelTaskBar store={store} reveal={revealTasks} onSettings={() => navigate('settings')} />
    {scanRoot && <Dialog title="扫描照片文件夹" onClose={() => { if (!busy) setScanRoot(null); }}><div className="stack"><p className="path-box filename">{scanRoot}</p><dl><dt>扫描范围</dt><dd>当前文件夹及子文件夹</dd><dt>文件处理</dt><dd>保留原位置</dd></dl></div>{(error || state.error) && <p role="alert" className="inline-error">{error || state.error}</p>}<div className="dialog-actions"><Button disabled={busy} onClick={() => setScanRoot(null)}>取消</Button><Button primary disabled={busy} onClick={() => attempt(startScan)}>{busy ? '正在打开' : '开始扫描'}</Button></div></Dialog>}
    {leaveTarget && <Dialog title="离开设置？" onClose={() => { if (!savingLeave) setLeaveTarget(null); }}><p>分析参数尚未保存。</p><div className="dialog-actions"><Button disabled={savingLeave} onClick={() => setLeaveTarget(null)}>继续编辑</Button><Button disabled={savingLeave} onClick={() => { setPage(leaveTarget); setSettingsDirty(false); setLeaveTarget(null); }}>放弃更改</Button><Button primary disabled={savingLeave} onClick={() => { setSavingLeave(true); setError(null); void (async () => { try { if (await saveSettings.current?.()) { setPage(leaveTarget); setSettingsDirty(false); setLeaveTarget(null); } else setError('保存未完成，请继续编辑并检查参数'); } catch (reason) { setError(String(reason)); } finally { setSavingLeave(false); } })(); }}>{savingLeave ? '正在保存' : '保存后返回'}</Button></div>{error && <p role="alert" className="inline-error">{error}</p>}</Dialog>}
    {closeRequested && <Dialog title="关闭伊人？" onClose={() => setCloseRequested(false)}><p>未保存的分析参数将被放弃。</p><div className="dialog-actions"><Button onClick={() => setCloseRequested(false)}>继续编辑</Button><Button primary onClick={() => attempt(async () => { await window.__TAURI__?.core.invoke('confirm_close'); })}>放弃更改并关闭</Button></div>{error && <p role="alert" className="inline-error">{error}</p>}</Dialog>}
  </div>;
}

const componentNames: Record<string, string> = { eyes: '眼部状态', sharpness: '清晰度', face: '面部质量', exposure: '曝光与构图', smile: '表情' };
function formatBytes(bytes: number) { return bytes >= 1048576 ? `${(bytes / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1024))} KB`; }
function formatTaken(value: string | null | undefined) {
  if (!value) return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' });
}

function PhotoDetails({ photo }: { photo: Photo }) {
  const analysis = readPhotoAnalysis(photo);
  const verdict = analysis.verdict ? verdictNames[analysis.verdict] : analysis.status === 'stale' ? '需要重新分析' : '尚未分析';
  const taken = formatTaken(photo.taken_at);
  const faces = analysis.current?.faces.length;
  const components = analysis.scoreBreakdown?.components.filter(item => item.score !== null && item.score !== undefined) ?? [];
  return <section className="photo-details">
    <h2 className="panel-title">照片信息</h2>
    <div className={`verdict-card verdict-${analysis.verdict ?? 'none'}`}>
      <div className="verdict-line"><span className="verdict-dot" aria-hidden="true" /><span>{verdict}</span>{analysis.score !== null && <span className="verdict-score tabular">{Math.round(analysis.score)}</span>}</div>
      {faces !== undefined && <span className="muted verdict-meta"><Icon name="face" />{faces ? `${faces} 张人脸` : '未检测到人脸'}</span>}
      {components.length > 0 && <details className="score-details"><summary>评分依据</summary><ul className="score-bars">{components.map(item => <li key={item.id}><span>{componentNames[item.id] ?? item.id}</span><span className="score-track" aria-hidden="true"><span style={{ width: `${Math.max(0, Math.min(100, item.score ?? 0))}%` }} /></span><span className="tabular">{Math.round(item.score ?? 0)}</span></li>)}</ul></details>}
    </div>
    <dl className="info-table">
      <dt>文件</dt><dd className="filename">{photo.filename}</dd>
      {taken && <><dt>拍摄</dt><dd className="tabular">{taken}</dd></>}
      <dt>尺寸</dt><dd className="tabular">{photo.width} × {photo.height} <span className="muted">· {(photo.width * photo.height / 1e6).toFixed(1)} MP</span></dd>
      <dt>格式</dt><dd>{photo.format.toUpperCase()} <span className="muted tabular">· {formatBytes(photo.size_bytes)}</span></dd>
    </dl>
    <details><summary>文件位置与分析提示</summary><div className="details-content"><p className="filename muted path-text">{photo.path}</p>{analysis.current?.warnings.map((message, index) => <p key={index} className="muted">{message}</p>)}</div></details>
  </section>;
}
