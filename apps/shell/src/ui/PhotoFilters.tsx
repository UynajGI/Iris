import { useEffect, useRef } from 'react';
import type { PhotoFilter } from '../types.js';
import { Button } from './components.js';
import { colorNames, decisionNames } from './MarkTools.js';

export const filterLabels: Partial<Record<keyof PhotoFilter, string>> = { decision: '标记', rating: '星标', color_label: '颜色', format: '格式', verdict: '建议' };
const verdicts = { recommend: '建议保留', review: '需要复核', reject_suggest: '建议移除' };
export function filterValue(key: keyof PhotoFilter, value: unknown): string {
  if (key === 'decision') return decisionNames[value as keyof typeof decisionNames];
  if (key === 'color_label') return colorNames[value as keyof typeof colorNames];
  if (key === 'verdict') return verdicts[value as keyof typeof verdicts];
  if (key === 'rating') return Number(value) === 0 ? '无星标' : `${value} 星`;
  return String(value).toUpperCase();
}

export function PhotoFilters({ filter, onChange, onClose }: { filter: PhotoFilter; onChange(filter: PhotoFilter): void; onClose(): void }) {
  const panel = useRef<HTMLElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    panel.current?.querySelector<HTMLSelectElement>('select')?.focus();
    const pointer = (event: PointerEvent) => { if (!panel.current?.contains(event.target as Node)) onClose(); };
    const escape = (event: KeyboardEvent) => { if (event.key === 'Escape') { event.preventDefault(); onClose(); } };
    document.addEventListener('pointerdown', pointer);
    document.addEventListener('keydown', escape);
    return () => { document.removeEventListener('pointerdown', pointer); document.removeEventListener('keydown', escape); previous?.focus(); };
  }, []);
  const change = (key: keyof PhotoFilter, value: string) => {
    const next = { ...filter, offset: 0 };
    delete next[key];
    onChange(value === '' ? next : { ...next, [key]: key === 'rating' ? Number(value) : value });
  };
  const choices = [
    ['decision', decisionNames], ['rating', { 0: '无星标', 1: '1 星', 2: '2 星', 3: '3 星', 4: '4 星', 5: '5 星' }],
    ['color_label', colorNames], ['verdict', verdicts],
    ['format', { jpeg: 'JPEG', png: 'PNG', webp: 'WebP', heic: 'HEIC', raw: 'RAW' }],
  ] as const;
  const active = choices.filter(([key]) => filter[key] !== undefined).length;
  return <section className="filter-popover" ref={panel} role="dialog" aria-label="筛选照片">
    <div className="popover-heading"><h2>筛选</h2>{active > 0 && <span className="muted tabular">{active} 项条件</span>}</div>
    <div className="popover-body">
      {choices.map(([key, options]) => <label key={key} className={filter[key] !== undefined ? 'field active' : 'field'}>{filterLabels[key]}<select value={filter[key] ?? ''} onChange={event => change(key, event.target.value)}><option value="">全部</option>{Object.entries(options).map(([value,label]) => <option key={value} value={value}>{label}</option>)}</select></label>)}
      <label className="field">文件名排序<select value={filter.descending ? 'desc' : 'asc'} onChange={event => onChange({ ...filter, sort: 'name', descending: event.target.value === 'desc', offset: 0 })}><option value="asc">升序</option><option value="desc">降序</option></select></label>
    </div>
    <div className="popover-actions"><Button className="quiet" disabled={!active} onClick={() => onChange({ limit: filter.limit ?? 200, sort: filter.sort ?? 'name', descending: filter.descending ?? false })}>清除筛选</Button><Button primary onClick={onClose}>完成</Button></div>
  </section>;
}
