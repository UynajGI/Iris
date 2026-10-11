import { Button } from './components.js';
import { Icon } from './Icon.js';

import type { PhotoWorkspaceModel } from './usePhotoWorkspace.js';

export function WorkspaceStatus({ model }: { model: PhotoWorkspaceModel }) {
  const { view, state, current, bulk, store, setStripOpen, stripOpen, group, offset, attempt, limit } = model;
  return (<footer className="statusbar">
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
  </footer>);
}

export function EmptyCanvas({ model }: { model: PhotoWorkspaceModel }) {
  const { running, activeFilters, attempt, store, limit, setScanRoot, state } = model;
  return (<div className="canvas-empty" role="status">{running ? <span className="activity-ring large" aria-hidden="true" /> : <Icon name={activeFilters ? 'filter' : 'library'} className="state-icon" />}<h2>{running ? '正在建立照片列表' : activeFilters ? '没有符合筛选的照片' : '暂无照片'}</h2>{!running && (activeFilters ? <Button onClick={() => attempt(() => store.setFilter({ limit }))}>清除筛选</Button> : <Button primary onClick={() => setScanRoot(state.project!.root)}><Icon name="refresh" />扫描照片文件夹</Button>)}</div>);
}
