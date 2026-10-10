import type { IrisStore } from '../store.js';
import type { Action } from '../types.js';
import { useIris } from '../react.js';
import { decisionNames } from './MarkTools.js';
import { Icon } from './Icon.js';

const decisionOrder: Action[] = ['pending', 'keep', 'flag', 'reject'];

export function LibraryNav({ store, onNavigate = () => {} }: { store: IrisStore; onNavigate?(): void }) {
  const state = useIris(store);
  const analyzing = state.progress?.kind === 'analysis' && ['running', 'paused'].includes(state.progress.state);
  const go = async (operation: () => Promise<void>) => {
    try { await operation(); onNavigate(); } catch { /* The store keeps the operation error visible. */ }
  };
  const allPhotos = !Object.entries(state.filter).some(([key, value]) => key !== 'offset' && key !== 'limit' && value !== undefined);
  const activeGroup = state.groupSession?.groupId;
  return <div className="library-content">
    <h2 className="nav-heading">照片</h2>
    <div className="nav-list">
      <button type="button" className="nav-item" aria-pressed={allPhotos} onClick={() => void go(() => store.setFilter({ limit: 200 }))}><Icon name="library" /><span>全部照片</span></button>
      {decisionOrder.map(value => <button type="button" key={value} className={`nav-item nav-${value}`} aria-pressed={state.filter.decision === value} onClick={() => void go(() => store.setFilter({ ...state.filter, decision: value, offset: 0 }))}><span className="decision-mark" aria-hidden="true" /><span>{decisionNames[value]}</span></button>)}
    </div>
    <h2 className="nav-heading">相似照片{state.groups.length > 0 && !analyzing && !state.reanalysisRequired && <span className="nav-count tabular">{state.groups.length}</span>}</h2>
    {analyzing || state.reanalysisRequired ? <p className="nav-note muted">{analyzing ? <><span className="activity-dot" aria-hidden="true" />正在更新分组</> : '分析与分组待更新'}</p>
      : state.groups.length ? <div className="nav-list">{state.groups.map((group, index) => <button type="button" key={group.id} className="nav-item" aria-pressed={activeGroup === group.id} onClick={() => void go(() => store.openGroup(group.id))}><Icon name="compare" /><span>第 {index + 1} 组</span><span className="nav-count tabular">{group.member_photo_ids.length}</span></button>)}</div>
      : <p className="nav-note muted">暂无相似组</p>}
  </div>;
}
