import { useState } from 'react';
import type { IrisStore } from '../store.js';
import type { Action, ColorLabel, MarkRequest, Photo } from '../types.js';
import { useIris } from '../react.js';
import { Button } from './components.js';
import { Icon } from './Icon.js';

export const colorNames: Record<ColorLabel, string> = { none: '无颜色', red: '红', yellow: '黄', green: '绿', blue: '蓝', purple: '紫' };
export const decisionNames: Record<Action, string> = { pending: '未标记', keep: '保留', flag: '已标记', reject: '标记移除' };
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
  return <section className="mark-tools stack"><h2>{bulk ? `已选 ${state.selectedIds.length} 张` : '照片标记'}</h2>
    {bulk ? <><label>决定<select value={decision} onChange={event => setDecision(event.target.value as Action | '')}><option value="">不更改</option>{Object.entries(decisionNames).map(([value,label]) => <option key={value} value={value}>{label}</option>)}</select></label>
      <label>星标<select value={rating} onChange={event => setRating(event.target.value)}><option value="">不更改</option>{[0,1,2,3,4,5].map(value => <option key={value} value={value}>{value === 0 ? '无星标' : `${value} 星`}</option>)}</select></label>
      <label>颜色<select value={color} onChange={event => setColor(event.target.value as ColorLabel | '')}><option value="">不更改</option>{Object.entries(colorNames).map(([value,label]) => <option key={value} value={value}>{label}</option>)}</select></label>
      <Button primary disabled={busy || (!decision && !rating && !color)} onClick={() => mark({ ...(decision ? { decision } : {}), ...(rating !== '' ? { rating: Number(rating) } : {}), ...(color ? { color_label: color } : {}) })}>应用到所选照片</Button>
      <Button onClick={() => store.select([])}>取消选择</Button>
    </> : <><div className="row">{(['keep','flag','reject','pending'] as Action[]).map(action => <Button key={action} disabled={!photo || busy} aria-pressed={photo?.decision === action} onClick={() => void run(() => state.groupSession ? store.decideGroup(action) : store.decide(action))}>{decisionNames[action]}</Button>)}</div>
      <div className="star-control" role="group" aria-label="星标">{[1,2,3,4,5].map(value => <button key={value} className="star-button" disabled={!photo || busy} aria-label={`${value} 星`} aria-pressed={(photo?.rating ?? 0) >= value} onClick={() => mark({ rating: value })}><Icon name="star" /></button>)}<Button disabled={!photo || busy || !photo.rating} onClick={() => mark({ rating: 0 })}>清除</Button></div>
      <div className="color-control" role="group" aria-label="颜色标签">{Object.entries(colorNames).map(([value,label]) => <button key={value} className={`color-button color-${value}`} disabled={!photo || busy} aria-label={label} aria-pressed={(photo?.color_label ?? 'none') === value} onClick={() => mark({ color_label: value as ColorLabel })}>{value !== 'none' && <span className="color-swatch" aria-hidden="true" />}{label}</button>)}</div>
    </>}
    <Button disabled={busy} onClick={() => void run(() => store.undo())}><Icon name="undo" />撤销</Button>
    {!bulk && <p className="muted shortcut-hint">W 保留 · S 移除 · P 标记<br />A / D 上一张 / 下一张 · Z 撤销</p>}
    {error && <p role="alert">{error}</p>}
  </section>;
}
