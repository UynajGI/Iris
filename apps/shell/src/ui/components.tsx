import { useEffect, useEffectEvent, useId, useRef, useState, type ButtonHTMLAttributes, type ReactNode } from 'react';
import type { IrisClient } from '../client.js';
import type { Photo } from '../types.js';
import { usePhotoUrl } from '../react.js';
import { Icon } from './Icon.js';

export function Button({ primary = false, className = '', ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { primary?: boolean }) {
  return <button type="button" className={`button ${primary ? 'primary' : ''} ${className}`} {...props} />;
}

function CloseButton({ onClick }: { onClick(): void }) {
  return <Button className="quiet icon-only" aria-label="关闭" title="关闭" onClick={onClick}><Icon name="close" /></Button>;
}

export function Popover({ title, children, onClose }: { title: string; children: ReactNode; onClose(): void }) {
  const panel = useRef<HTMLElement>(null);
  const restoreFocus = useRef(true);
  const close = useEffectEvent(onClose);
  const id = useId();
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    panel.current?.querySelector<HTMLElement>('.popover-body button,.popover-body select,.popover-body input')?.focus();
    const outside = (event: PointerEvent) => { if (!panel.current?.contains(event.target as Node)) { restoreFocus.current = false; close(); } };
    document.addEventListener('pointerdown', outside);
    return () => { document.removeEventListener('pointerdown', outside); if (restoreFocus.current && previous?.isConnected) previous.focus(); };
  }, []);
  return <section ref={panel} className="filter-popover" role="dialog" aria-labelledby={id} onBlur={event => { if (event.relatedTarget && !event.currentTarget.contains(event.relatedTarget)) { restoreFocus.current = false; onClose(); } }} onKeyDown={event => { event.stopPropagation(); if (event.key === 'Escape') { event.preventDefault(); onClose(); } }}>
    <div className="popover-heading"><h2 id={id}>{title}</h2><CloseButton onClick={onClose} /></div><div className="popover-body">{children}</div>
  </section>;
}

export function Dialog({ title, children, onClose }: { title: string; children: ReactNode; onClose(): void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const id = useId();
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const node = dialog.current;
    node?.showModal();
    node?.querySelector<HTMLButtonElement>('.dialog-actions button:not(.primary):not(:disabled)')?.focus();
    return () => { node?.close(); if (previous?.isConnected) previous.focus(); };
  }, []);
  return <dialog ref={dialog} aria-labelledby={id} onCancel={event => { event.preventDefault(); onClose(); }} onKeyDown={event => event.stopPropagation()}>
    <header className="dialog-heading"><h2 id={id}>{title}</h2><CloseButton onClick={onClose} /></header>{children}
  </dialog>;
}

export function PhotoImage({ client, photo, kind = 'thumb' }: { client: IrisClient; photo: Photo; kind?: 'thumb' | 'preview' | 'original' }) {
  const media = usePhotoUrl(client, photo.id, kind);
  const [failedUrl, setFailedUrl] = useState<string | null>(null);
  const [loadedUrl, setLoadedUrl] = useState<string | null>(null);
  const failed = !!media.error || (!!media.url && failedUrl === media.url);
  // Cached images may finish before React attaches onLoad; read completion from the node.
  const observe = (node: HTMLImageElement | null) => { if (node?.complete && node.naturalWidth && media.url && loadedUrl !== media.url) setLoadedUrl(media.url); };
  return media.url && !failed ? <img ref={observe} className={loadedUrl === media.url ? 'loaded' : 'loading'} src={media.url} alt={photo.filename} loading={kind === 'thumb' ? 'lazy' : 'eager'} decoding="async" draggable={false} onLoad={() => setLoadedUrl(media.url)} onError={() => setFailedUrl(media.url)} />
    : <span className={`photo-placeholder ${failed ? 'unavailable' : 'loading'}`} role="img" aria-label={`${photo.filename}：${failed ? '照片暂不可用' : '正在载入'}`}>{failed ? <><Icon name="image" />照片暂不可用</> : null}</span>;
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
