import type { IrisStore } from '../store.js';
import type { Action } from '../types.js';
import { useIris } from '../react.js';
import { Button } from './components.js';
import { decisionNames } from './MarkTools.js';

export function LibraryNav({ store, onNavigate = () => {} }: { store: IrisStore; onNavigate?(): void }) {
  const state = useIris(store);
  const analyzing = state.progress?.kind === 'analysis' && ['running', 'paused'].includes(state.progress.state);
  const go = async (operation: () => Promise<void>) => {
    try { await operation(); onNavigate(); } catch { /* The store keeps the operation error visible. */ }
  };
  return <div className="library-content"><h2>照片</h2><Button onClick={() => void go(() => store.setFilter({ limit: 200 }))}>全部照片</Button>
    {Object.entries(decisionNames).map(([value,label]) => <Button key={value} aria-pressed={state.filter.decision === value} onClick={() => void go(() => store.setFilter({ ...state.filter, decision: value as Action, offset: 0 }))}>{label}</Button>)}
    <h2>相似照片</h2>{analyzing || state.reanalysisRequired ? <p className="muted">{analyzing ? '正在更新分组' : '分析与分组待更新'}</p> : state.groups.length ? state.groups.map((group,index) => <Button key={group.id} onClick={() => void go(() => store.openGroup(group.id))}>第 {index + 1} 组 · {group.member_photo_ids.length} 张</Button>) : <p className="muted">暂无相似组</p>}
  </div>;
}
