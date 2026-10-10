import { useState } from 'react';
import type { IrisStore } from '../store.js';
import type { Action, ColorLabel, MarkRequest, Photo } from '../types.js';
import { useIris } from '../react.js';
import { Button } from './components.js';
import { Icon, type IconName } from './Icon.js';

export const colorNames: Record<ColorLabel, string> = { none: '无颜色', red: '红', yellow: '黄', green: '绿', blue: '蓝', purple: '紫' };
export const decisionNames: Record<Action, string> = { pending: '未标记', keep: '保留', flag: '已标记', reject: '标记移除' };
const decisionControls: { action: Action; icon: IconName; key?: string }[] = [
  { action: 'keep', icon: 'done', key: 'W' }, { action: 'flag', icon: 'flag', key: 'P' },
  { action: 'reject', icon: 'close', key: 'S' }, { action: 'pending', icon: 'undo' },
];
export function MarkTools({ store, photo }: { store: IrisStore; photo: Photo | undefined }) {
  const state = useIris(store);
  const bulk = !state.groupSession && state.selectedIds.length > 0;
  const [decision, setDecision] = useState<Action | ''>('');
  const [rating, setRating] = useState('');
  const [color, setColor] = useState<ColorLabel | ''>('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const run = async (operation: () => Promise<void>) => {
    if (busy) return;
    setBusy(true); setError(null);
    try { await operation(); } catch (reason) { setError(String(reason)); } finally { setBusy(false); }
  };
  const mark = (fields: Omit<MarkRequest, 'photo_ids'>) => void run(() => store.mark(fields));
  return <section className="mark-tools" aria-busy={busy}>
    <header className="panel-heading"><h2>{bulk ? <>已选 <span className="tabular">{state.selectedIds.length}</span> 张</> : state.groupSession ? '相似组标记' : '照片标记'}</h2>
      <Button className="quiet icon-only" disabled={busy} aria-label="撤销" title="撤销 (Z)" onClick={() => void run(() => store.undo())}><Icon name="undo" /></Button></header>
    {bulk ? <div className="bulk-fields">
      <label className="field">决定<select value={decision} onChange={event => setDecision(event.target.value as Action | '')}><option value="">不更改</option>{Object.entries(decisionNames).map(([value,label]) => <option key={value} value={value}>{label}</option>)}</select></label>
      <label className="field">星标<select value={rating} onChange={event => setRating(event.target.value)}><option value="">不更改</option>{[0,1,2,3,4,5].map(value => <option key={value} value={value}>{value === 0 ? '无星标' : `${value} 星`}</option>)}</select></label>
      <label className="field">颜色<select value={color} onChange={event => setColor(event.target.value as ColorLabel | '')}><option value="">不更改</option>{Object.entries(colorNames).map(([value,label]) => <option key={value} value={value}>{label}</option>)}</select></label>
      <div className="bulk-actions"><Button primary disabled={busy || (!decision && !rating && !color)} onClick={() => mark({ ...(decision ? { decision } : {}), ...(rating !== '' ? { rating: Number(rating) } : {}), ...(color ? { color_label: color } : {}) })}>应用到 {state.selectedIds.length} 张照片</Button>
        <Button className="quiet" onClick={() => store.select([])}>取消选择</Button></div>
    </div> : <>
      <div className="decision-control" role="group" aria-label="决定">{decisionControls.map(({ action, icon, key }) => <button type="button" key={action} className={`decision-button decision-${action}`} disabled={!photo || busy} aria-pressed={photo?.decision === action} onClick={() => void run(() => state.groupSession ? store.decideGroup(action) : store.decide(action))}>
        <Icon name={icon} /><span>{decisionNames[action]}</span>{key && <kbd aria-hidden="true">{key}</kbd>}</button>)}</div>
      <div className="mark-row"><span className="mark-label">星标</span><div className="star-control" role="group" aria-label="星标">{[1,2,3,4,5].map(value => <button type="button" key={value} className="star-button" disabled={!photo || busy} aria-label={`${value} 星`} aria-pressed={(photo?.rating ?? 0) >= value} onClick={() => mark({ rating: value })}><Icon name="star" /></button>)}
        <Button className="quiet star-clear" disabled={!photo || busy || !photo.rating} onClick={() => mark({ rating: 0 })}>清除</Button></div></div>
      <div className="mark-row"><span className="mark-label">颜色</span><div className="color-control" role="group" aria-label="颜色标签">{Object.entries(colorNames).map(([value,label]) => <button type="button" key={value} className={`color-button color-${value}`} disabled={!photo || busy} aria-label={label} title={label} aria-pressed={(photo?.color_label ?? 'none') === value} onClick={() => mark({ color_label: value as ColorLabel })}>{value !== 'none' ? <span className="color-swatch" aria-hidden="true" /> : <span className="color-empty-swatch" aria-hidden="true" />}<span className="color-name">{label}</span></button>)}</div></div>
    </>}
    {!bulk && <details className="shortcut-hint"><summary>键盘快捷键</summary><dl className="shortcut-list"><dt><kbd>W</kbd></dt><dd>保留并前进</dd><dt><kbd>S</kbd></dt><dd>标记移除并前进</dd><dt><kbd>P</kbd></dt><dd>标记</dd><dt><kbd>A</kbd> <kbd>D</kbd></dt><dd>上一张 / 下一张</dd><dt><kbd>空格</kbd></dt><dd>单张 / 总览</dd><dt><kbd>Z</kbd></dt><dd>撤销</dd></dl></details>}
    {error && <p className="inline-error" role="alert">{error}</p>}
  </section>;
}
