import { Button } from './components.js';
import { Icon } from './Icon.js';

import type { PhotoWorkspaceModel } from './usePhotoWorkspace.js';

export function WorkspaceToolbar({ model }: { model: PhotoWorkspaceModel }) {
  const { libraryOpen, toggleLibrary, view, setView, current, running, state, group, attempt, store, tileInfo, setTileInfo, showFaces, hasAnalysis, setShowFaces, progress, rightOpen, setRightOpen } = model;
  return (<div className="toolbar">
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
  </div>);
}

export function GroupNavigation({ model }: { model: PhotoWorkspaceModel }) {
  const { group, setView, groupIndex, attempt, store, state, groupPosition } = model;
  if (!group) return null;
  return (<>{group && <div className="subbar" aria-label="相似组导航">
    <button type="button" className="sub-button" onClick={() => setView('grid')}><Icon name="back" />返回网格</button>
    <span className="tool-divider" aria-hidden="true" />
    <button type="button" className="sub-icon" aria-label="上一组" title="上一组" disabled={group.loading || groupIndex <= 0} onClick={() => attempt(() => store.moveGroup(-1))}><Icon name="chevron-left" /></button>
    <span className="tabular subbar-title">相似组 {groupIndex + 1}<span className="muted"> / {state.groups.length}</span></span>
    <button type="button" className="sub-icon" aria-label="下一组" title="下一组" disabled={group.loading || groupIndex >= state.groups.length - 1} onClick={() => attempt(() => store.moveGroup(1))}><Icon name="chevron-right" /></button>
    <span className="muted tabular">{group.memberIds.length} 张 · 正在对比 {group.comparisonIds.length} 张</span>
    <span className="spacer" />
    <button type="button" className="sub-button" disabled={group.loading || groupPosition <= 0} onClick={() => store.moveGroupPhoto(-1)}><Icon name="chevron-left" />上一张</button>
    <button type="button" className="sub-button" disabled={group.loading || groupPosition >= group.memberIds.length - 1} onClick={() => store.moveGroupPhoto(1)}>下一张<Icon name="chevron-right" /></button>
  </div>}</>);
}
