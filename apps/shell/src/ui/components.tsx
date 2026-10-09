import { useEffect, useId, useRef, useState, type ButtonHTMLAttributes, type ReactNode } from 'react';
import type { IrisClient } from '../client.js';
import type { Photo } from '../types.js';
import { usePhotoUrl } from '../react.js';

export function Button({ primary = false, className = '', ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { primary?: boolean }) {
  return <button type="button" className={`button ${primary ? 'primary' : ''} ${className}`} {...props} />;
}

export function Popover({ title, children, onClose }: { title: string; children: ReactNode; onClose(): void }) {
  const panel = useRef<HTMLElement>(null);
  const id = useId();
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    panel.current?.querySelector<HTMLElement>('button,select,input')?.focus();
    const outside = (event: PointerEvent) => { if (!panel.current?.contains(event.target as Node)) onClose(); };
    document.addEventListener('pointerdown', outside);
    return () => { document.removeEventListener('pointerdown', outside); previous?.focus(); };
  }, []);
  return <section ref={panel} className="filter-popover stack" role="dialog" aria-labelledby={id} onKeyDown={event => { event.stopPropagation(); if (event.key === 'Escape') { event.preventDefault(); onClose(); } }}><div className="row"><h2 id={id}>{title}</h2><Button onClick={onClose}>关闭</Button></div>{children}</section>;
}

export function Dialog({ title, children, onClose }: { title: string; children: ReactNode; onClose(): void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const id = useId();
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    dialog.current?.showModal();
    dialog.current?.querySelector<HTMLButtonElement>('.dialog-actions button:not(.primary)')?.focus();
    return () => { dialog.current?.close(); previous?.focus(); };
  }, []);
  return <dialog ref={dialog} aria-labelledby={id} onCancel={event => { event.preventDefault(); onClose(); }} onKeyDown={event => event.stopPropagation()}>
    <header className="dialog-heading"><h2 id={id}>{title}</h2><Button onClick={onClose}>关闭</Button></header>{children}
  </dialog>;
}

export function PhotoImage({ client, photo, kind = 'thumb' }: { client: IrisClient; photo: Photo; kind?: 'thumb' | 'preview' | 'original' }) {
  const media = usePhotoUrl(client, photo.id, kind);
  return media.url ? <img src={media.url} alt={photo.filename} loading={kind === 'thumb' ? 'lazy' : 'eager'} draggable={false} />
    : <span className="photo-placeholder" role="status">{media.error ? '照片暂不可用' : '载入中'}</span>;
}

/** Integer-only editing with an unboxed value until explicitly activated. */
export function IntegerControl({ label, value, min = 0, max = 100, slider = true, onChange }: {
  label: string; value: number; min?: number; max?: number; slider?: boolean; onChange(value: number): void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState('');
  const [error, setError] = useState<string | null>(null);
  const id = useId();
  const commit = () => {
    const parsed = Number(draft);
    if (!/^-?\d+$/.test(draft) || !Number.isSafeInteger(parsed) || parsed < min || parsed > max) {
      setError(`请输入 ${min}–${max} 的整数`);
      return;
    }
    onChange(parsed);
    setError(null);
    setEditing(false);
  };
  return <div className="integer-control"><label htmlFor={id}>{label}</label>
    {slider && <input id={id} aria-label={label} type="range" min={min} max={max} step="1" value={Math.round(value)} onChange={event => onChange(Number(event.target.value))} />}
    {editing ? <input className="integer-editor" aria-label={`${label}数值`} aria-invalid={!!error} aria-describedby={error ? `${id}-error` : undefined} autoFocus inputMode="numeric" value={draft} onChange={event => { setDraft(event.target.value); setError(null); }} onBlur={commit} onKeyDown={event => {
      if (event.key === 'Enter') { event.preventDefault(); commit(); }
      if (event.key === 'Escape') { event.preventDefault(); setError(null); setEditing(false); }
    }} /> : <Button className="integer-value" aria-label={`编辑${label}：${Math.round(value)}`} onClick={() => { setDraft(String(Math.round(value))); setEditing(true); }}>{Math.round(value)}</Button>}
    {error && <p id={`${id}-error`} className="integer-error" role="alert">{error}</p>}
  </div>;
}
