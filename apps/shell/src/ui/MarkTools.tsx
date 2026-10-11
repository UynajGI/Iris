import { useState } from 'react';
import type { IrisStore } from '../store.js';
import type { Action, ColorLabel, MarkRequest, Photo } from '../types.js';
import { useIris } from '../react.js';
import { Button } from './components.js';
import { Icon, type IconName } from './Icon.js';

export const colorNames: Record<ColorLabel, string> = { none: '无颜色', red: '红', yellow: '黄', green: '绿', blue: '蓝', purple: '紫' };
export const decisionNames: Record<Action, string> = { pending: '未标记', keep: '保留', flag: '已标记', reject: '标记移除' };
const decisionControls: { action: Exclude<Action, 'pending'>; icon: IconName; key?: string }[] = [
  { action: 'keep', icon: 'done', key: 'W' }, { action: 'flag', icon: 'flag', key: 'P' },
  { action: 'reject', icon: 'close', key: 'S' },
];
export function MarkTools({ store, photo }: { store: IrisStore; photo: Photo | undefined }) {
  const state = useIris(store);
  const bulk = !state.groupSession && state.selectedIds.length > 0;
  const [decision, setDecision] = useState<Action | ''>('');
  const [rating, setRating] = useState('');
  const [color, setColor] = useState<ColorLabel | ''>('');
  const [error, setError] = useState<string | null>(null);
  const unavailable = !!state.groupSession?.loading;
  const mark = (fields: Omit<MarkRequest, 'photo_ids'>) => {
    if (unavailable) return;
    setError(null);
    void store.mark(fields).catch(reason => setError(`标记保存失败：${String(reason)}`));
  };
  return <section className={`mark-tools mark-tools-toolbar${bulk ? ' is-bulk' : ''}`} aria-label={bulk ? `批量标记 ${state.selectedIds.length} 张照片` : state.groupSession ? '相似组标记工具栏' : '照片标记工具栏'} aria-busy={unavailable}>
    {bulk && <span className="mark-target tabular">已选 {state.selectedIds.length} 张</span>}
    {bulk ? <BulkMarkFields count={state.selectedIds.length} decision={decision} setDecision={setDecision} rating={rating} setRating={setRating} color={color} setColor={setColor} unavailable={unavailable} mark={mark} cancel={() => store.select([])} /> : <>
      <DecisionControl photo={photo} unavailable={unavailable} toggle={action => { setError(null); void store.toggleDecision(action).catch(reason => setError(`标记保存失败：${String(reason)}`)); }} />
      <RatingControl photo={photo} unavailable={unavailable} mark={mark} />
      <ColorControl photo={photo} unavailable={unavailable} mark={mark} />
    </>}
    {error && <p className="inline-error" role="alert">{error}</p>}
  </section>;
}

interface MarkControlProps { photo: Photo | undefined; unavailable: boolean; mark(fields: Omit<MarkRequest, "photo_ids">): void }
function DecisionControl({ photo, unavailable, toggle }: Omit<MarkControlProps, "mark"> & { toggle(action: Exclude<Action, "pending">): void }) {
  return (<div className="decision-control" role="group" aria-label="决定">{decisionControls.map(({ action, icon, key }) => <button type="button" key={action} className={`decision-button decision-${action}`} aria-label={decisionNames[action]} title={`${decisionNames[action]}${key ? ` (${key})` : ''} · 再点取消`} disabled={!photo || unavailable} aria-pressed={photo?.decision === action} onClick={() => toggle(action)}>
    <Icon name={icon} /><span>{decisionNames[action]}</span>{key && <kbd aria-hidden="true">{key}</kbd>}</button>)}</div>);
}
function RatingControl({ photo, unavailable, mark }: MarkControlProps) {
  return (<div className="mark-row"><span className="mark-label">星标</span><div className="star-control" role="group" aria-label="星标">{[1, 2, 3, 4, 5].map(value => <button type="button" key={value} className="star-button" disabled={!photo || unavailable} aria-label={`${value} 星`} title={`${value} 星 · 再点取消`} aria-pressed={(photo?.rating ?? 0) >= value} onClick={() => mark({ rating: photo?.rating === value ? 0 : value })}><Icon name="star" /></button>)}</div></div>);
}
function ColorControl({ photo, unavailable, mark }: MarkControlProps) {
  return (<div className="mark-row"><span className="mark-label">颜色</span><div className="color-control" role="group" aria-label="颜色标签">{Object.entries(colorNames).map(([value, label]) => <button type="button" key={value} className={`color-button color-${value}`} disabled={!photo || unavailable} aria-label={label} title={label} aria-pressed={(photo?.color_label ?? 'none') === value} onClick={() => mark({ color_label: value as ColorLabel })}>{value !== 'none' ? <span className="color-swatch" aria-hidden="true" /> : <span className="color-empty-swatch" aria-hidden="true" />}<span className="color-name">{label}</span></button>)}</div></div>);
}
interface BulkFieldsProps { count: number; decision: Action | ""; rating: string; color: ColorLabel | ""; unavailable: boolean; setDecision(value: Action | ""): void; setRating(value: string): void; setColor(value: ColorLabel | ""): void; mark(fields: Omit<MarkRequest, "photo_ids">): void; cancel(): void }
function BulkMarkFields({ count, decision, setDecision, rating, setRating, color, setColor, unavailable, mark, cancel }: BulkFieldsProps) {
  return (<div className="bulk-fields">
    <label className="field">决定<select value={decision} onChange={event => setDecision(event.target.value as Action | '')}><option value="">不更改</option>{Object.entries(decisionNames).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select></label>
    <label className="field">星标<select value={rating} onChange={event => setRating(event.target.value)}><option value="">不更改</option>{[0, 1, 2, 3, 4, 5].map(value => <option key={value} value={value}>{value === 0 ? '无星标' : `${value} 星`}</option>)}</select></label>
    <label className="field">颜色<select value={color} onChange={event => setColor(event.target.value as ColorLabel | '')}><option value="">不更改</option>{Object.entries(colorNames).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select></label>
    <div className="bulk-actions"><Button primary disabled={unavailable || (!decision && !rating && !color)} onClick={() => mark({ ...(decision ? { decision } : {}), ...(rating !== '' ? { rating: Number(rating) } : {}), ...(color ? { color_label: color } : {}) })}>应用到 {count} 张照片</Button>
      <Button className="quiet" onClick={cancel}>取消选择</Button></div>
  </div>);
}
