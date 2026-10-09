import { useCallback, useEffect, useRef, useState } from 'react';
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
import './app.css';

const decisionNames: Record<Action, string> = { pending: '未标记', keep: '保留', flag: '已标记', reject: '标记移除' };
const verdictNames = { recommend: '建议保留', review: '需要复核', reject_suggest: '建议移除' };

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

  return <div className="app">
    <header className="app-header"><span className="brand">伊人</span><Button onClick={() => navigate('projects')}><Icon name="folder" />项目</Button>
      <span className="project-name">{state.project?.name ?? ''}</span>
      {state.project && <Button onClick={() => navigate('photos')}>选片</Button>}
      <Button onClick={() => setRevealTasks(value => value + 1)}>任务</Button>
      <Button aria-pressed={page === 'settings'} onClick={() => navigate('settings')}><Icon name="settings" />设置</Button>
    </header>
    {(error || state.error) && <div role="alert" className="error-bar">{error || state.error}</div>}
    {state.connection === 'disconnected' && <div role="status" className="error-bar">服务连接已断开，正在等待恢复</div>}
    {page === 'projects' && inaccessible && <main className="page"><div className="page-content stack"><h1>文件夹无法访问</h1><h2>{inaccessible.project.name}</h2><p className="filename">{inaccessible.project.root}</p><p className="muted">请检查连接、权限或文件夹位置。</p><div className="row"><Button primary onClick={() => attempt(() => openRecent(inaccessible.project))}>重试</Button><Button onClick={() => { setInaccessible(null); store.clearError(); }}>返回项目列表</Button></div><details><summary>详细原因</summary><p>{inaccessible.reason}</p></details></div></main>}
    {page === 'projects' && !inaccessible && <main className="page"><div className="page-content stack"><div className="row"><h1>最近项目</h1><Button primary onClick={() => attempt(pickFolder)}>打开照片文件夹</Button></div>
      {state.projects.length === 0 ? <p className="muted">暂无项目</p> : <div className="project-list">{state.projects.map(project => <div key={project.id} className="project-row">
        <button className="project-open" onClick={() => attempt(() => openRecent(project))}><strong>{project.name}</strong><span className="muted">{project.root}</span></button>
        <Button onClick={() => attempt(() => store.hideRecentProject(project.id))}>移出列表</Button>
      </div>)}</div>}
    </div></main>}
    {page === 'settings' && <main className="page"><div className="page-content"><h1>设置</h1><section className="settings-section"><h2>外观</h2>
      <label>字号<select value={appearance.size} onChange={event => setAppearance({ ...appearance, size: event.target.value as Appearance['size'] })}><option value="small">小</option><option value="medium">中</option><option value="large">大</option></select></label>
      <label>界面密度<select value={appearance.density} onChange={event => setAppearance({ ...appearance, density: event.target.value as Appearance['density'] })}><option value="compact">紧凑</option><option value="standard">标准</option><option value="relaxed">宽松</option></select></label>
    </section><AnalysisSettings store={store} onDirty={setSettingsDirty} onSaveReady={registerSave} /><CacheSettings store={store} /></div></main>}
    {page === 'photos' && state.project && <main className={`workspace ${state.reviewOpen || group ? 'review' : ''}`}>
      {!state.reviewOpen && !group && <nav className="library" aria-label="照片分类"><LibraryNav store={store} /></nav>}
      <section ref={photoArea} className="photo-area" aria-label="照片工作区"><div className="photo-toolbar">
        <div className={`filter-anchor ${state.reviewOpen || group ? '' : 'narrow-only'}`}><Button aria-expanded={directoryOpen} onClick={() => setDirectoryOpen(true)}><Icon name="folder" />目录</Button>{directoryOpen && <Popover title="照片目录" onClose={() => setDirectoryOpen(false)}><LibraryNav store={store} onNavigate={() => setDirectoryOpen(false)} /></Popover>}</div>
        <Button onClick={() => { if (group) store.closeGroup(); else store.toggleReview(); }}>{group ? '返回总览' : state.reviewOpen ? '总览' : '单张'}</Button>
        <div className="filter-anchor"><Button aria-expanded={filterOpen} onClick={() => setFilterOpen(true)}><Icon name="filter" />筛选</Button>
          {filterOpen && <PhotoFilters filter={state.filter} onChange={filter => attempt(() => store.setFilter(filter))} onClose={() => setFilterOpen(false)} />}
        </div>
        {Object.entries(filterLabels).map(([key, label]) => {
          const typed = key as keyof typeof state.filter;
          const value = state.filter[typed];
          return value === undefined ? null : <Button key={key} aria-label={`清除${label}筛选`} onClick={() => attempt(() => { const next = { ...state.filter, offset: 0 }; delete next[typed]; return store.setFilter(next); })}>{filterValue(typed, value)} · 清除</Button>;
        })}
        <span className="spacer" />
        <Button disabled={!!running || !!progress?.root_unavailable} onClick={() => attempt(() => store.run('analyze'))}>开始分析</Button>
        <Button disabled={!!running} onClick={() => setScanRoot(state.project!.root)}>重新扫描</Button>
      </div>
      {group && <div className="photo-toolbar" aria-label="相似组导航"><Button disabled={group.loading || groupIndex <= 0} onClick={() => attempt(() => store.moveGroup(-1))}>上一组</Button><span className="muted">第 {groupIndex + 1} / {state.groups.length} 组 · {group.memberIds.length} 张</span><Button disabled={group.loading || groupIndex >= state.groups.length - 1} onClick={() => attempt(() => store.moveGroup(1))}>下一组</Button><span className="spacer" /><Button disabled={group.loading || group.memberIds.indexOf(group.focusedId!) <= 0} onClick={() => store.moveGroupPhoto(-1)}>上一张</Button><Button disabled={group.loading || group.memberIds.indexOf(group.focusedId!) >= group.memberIds.length - 1} onClick={() => store.moveGroupPhoto(1)}>下一张</Button></div>}
      {group?.loading && <p role="status" className="photo-toolbar">正在载入相似照片</p>}
      {group && !group.loading && !group.comparisonIds.length && <p className="photo-toolbar">从下方选择要对比的照片</p>}
      {group ? <><div className="compare-grid">{group.photos.filter(photo => group.comparisonIds.includes(photo.id)).map(photo => <button key={photo.id} className="compare-image" aria-label={`查看 ${photo.filename}`} aria-pressed={photo.id === group.focusedId} onClick={() => store.focusGroupPhoto(photo.id)}><PhotoImage client={store.client} photo={photo} kind="preview" /></button>)}</div>
        <div className="filmstrip">{group.photos.map(photo => <button key={photo.id} data-photo-id={photo.id} className="photo-button" aria-label={photo.filename} aria-pressed={group.comparisonIds.includes(photo.id)} onClick={() => { const ids = group.comparisonIds.includes(photo.id) ? group.comparisonIds.filter(id => id !== photo.id) : [...group.comparisonIds, photo.id]; store.setComparisonCandidates(ids); }}><PhotoImage client={store.client} photo={photo} /></button>)}</div></>
      : state.reviewOpen && current ? <><div className="review-image"><PhotoImage client={store.client} photo={current} kind="preview" /></div><div className="filmstrip">{state.photos.map(photo => <button key={photo.id} data-photo-id={photo.id} className="photo-button" aria-label={photo.filename} aria-pressed={photo.id === current.id} onClick={() => store.focus(photo.id)}><PhotoImage client={store.client} photo={photo} /></button>)}</div></>
      : state.photos.length ? <div className="photo-grid">{state.photos.map(photo => <article key={photo.id} data-photo-id={photo.id} className={`photo-tile ${photo.id === state.focusedId ? 'current' : ''}`}>
        <button className="photo-button" onClick={() => store.focus(photo.id)} onDoubleClick={() => { store.focus(photo.id); if (!state.reviewOpen) store.toggleReview(); }} aria-label={`查看 ${photo.filename}`}><PhotoImage client={store.client} photo={photo} /></button>
        <label className="photo-check"><input type="checkbox" aria-label={`选择 ${photo.filename}`} checked={state.selectedIds.includes(photo.id)} onChange={event => store.select(event.target.checked ? [...state.selectedIds, photo.id] : state.selectedIds.filter(id => id !== photo.id))} /></label>
        <div className="photo-caption"><span title={photo.filename}>{photo.filename}</span><span>{decisionNames[photo.decision]}</span></div>
        <div className="photo-marks"><span>{photo.rating ? `${photo.rating} 星` : '无星标'}</span><span className="photo-color">{photo.color_label && photo.color_label !== 'none' && <span className={`color-dot color-${photo.color_label}`} aria-hidden="true" />}{colorNames[photo.color_label ?? 'none']}</span></div>
      </article>)}</div> : <div className="empty"><h2>{running ? '正在建立照片列表' : '暂无照片'}</h2></div>}
      {!group && <div className="photo-toolbar"><Button disabled={(state.filter.offset ?? 0) === 0} onClick={() => attempt(() => store.setFilter({ ...state.filter, offset: Math.max(0, (state.filter.offset ?? 0) - (state.filter.limit ?? 200)) }))}>上一页</Button><span className="muted">本页 {state.photos.length} 张</span><Button disabled={state.photos.length < (state.filter.limit ?? 200)} onClick={() => attempt(() => store.setFilter({ ...state.filter, offset: (state.filter.offset ?? 0) + (state.filter.limit ?? 200) }))}>下一页</Button></div>}
      </section>
      <aside className="inspector"><div className="inspector-tools"><MarkTools key={state.project.id} store={store} photo={current} /></div><div className="inspector-secondary">
        {current && <PhotoDetails photo={current} />}
        <FileWorkflows key={state.project.id} store={store} />
      </div></aside>
    </main>}
    <TaskBar store={store} reveal={revealTasks} onSettings={() => navigate('settings')} />
    <ModelTaskBar store={store} reveal={revealTasks} onSettings={() => navigate('settings')} />
    {scanRoot && <Dialog title="扫描照片文件夹" onClose={() => { if (!busy) setScanRoot(null); }}><div className="stack"><p className="filename">{scanRoot}</p><dl><dt>扫描范围</dt><dd>当前文件夹及子文件夹</dd><dt>文件处理</dt><dd>保留原位置</dd></dl></div>{(error || state.error) && <p role="alert">{error || state.error}</p>}<div className="dialog-actions"><Button disabled={busy} onClick={() => setScanRoot(null)}>取消</Button><Button primary disabled={busy} onClick={() => attempt(startScan)}>{busy ? '正在打开' : '开始扫描'}</Button></div></Dialog>}
    {leaveTarget && <Dialog title="离开设置？" onClose={() => { if (!savingLeave) setLeaveTarget(null); }}><p>分析参数尚未保存。</p><div className="dialog-actions"><Button disabled={savingLeave} onClick={() => setLeaveTarget(null)}>继续编辑</Button><Button disabled={savingLeave} onClick={() => { setPage(leaveTarget); setSettingsDirty(false); setLeaveTarget(null); }}>放弃更改</Button><Button primary disabled={savingLeave} onClick={() => { setSavingLeave(true); setError(null); void (async () => { try { if (await saveSettings.current?.()) { setPage(leaveTarget); setSettingsDirty(false); setLeaveTarget(null); } else setError('保存未完成，请继续编辑并检查参数'); } catch (reason) { setError(String(reason)); } finally { setSavingLeave(false); } })(); }}>{savingLeave ? '正在保存' : '保存后返回'}</Button></div>{error && <p role="alert">{error}</p>}</Dialog>}
    {closeRequested && <Dialog title="关闭伊人？" onClose={() => setCloseRequested(false)}><p>未保存的分析参数将被放弃。</p><div className="dialog-actions"><Button onClick={() => setCloseRequested(false)}>继续编辑</Button><Button primary onClick={() => attempt(async () => { await window.__TAURI__?.core.invoke('confirm_close'); })}>放弃更改并关闭</Button></div>{error && <p role="alert">{error}</p>}</Dialog>}
  </div>;
}

function PhotoDetails({ photo }: { photo: Photo }) {
  const analysis = readPhotoAnalysis(photo);
  return <section className="stack"><h3 className="filename">{photo.filename}</h3><p>{analysis.verdict ? verdictNames[analysis.verdict] : analysis.status === 'stale' ? '需要重新分析' : '尚未分析'}</p>
    <details><summary>照片详情</summary><div className="details-content"><dl><dt>尺寸</dt><dd>{photo.width} × {photo.height}</dd><dt>格式</dt><dd>{photo.format}</dd><dt>评分</dt><dd>{analysis.score === null ? '—' : Math.round(analysis.score)}</dd><dt>位置</dt><dd>{photo.path}</dd></dl>{analysis.current?.warnings.map((message, index) => <p key={index}>{message}</p>)}</div></details>
  </section>;
}
