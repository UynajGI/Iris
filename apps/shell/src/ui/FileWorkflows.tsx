import { useState } from 'react';
import type { IrisStore } from '../store.js';
import { useIris } from '../react.js';
import { selectExportCsv, selectProjectFolder } from '../native-dialog.js';
import { Button, Dialog } from './components.js';
import { Icon } from './Icon.js';

type Flow = 'export' | 'quarantine' | 'restore' | null;
const recoveryStates: Record<string, string> = { preview: '待隔离', committed: '已隔离', restored: '已恢复', restoring: '恢复中断', restore_rollback: '待完成回滚', restore_rollback_failed: '回滚未完成', interrupted: '操作中断' };
function recoverySummary(reason: string, state?: string) {
  if (state?.startsWith('restore_rollback') || state === 'restoring') return '恢复未完成，重试会先接续回滚';
  if (reason.includes('completed operations rolled back')) return '恢复未完成，已执行的操作已回滚';
  return '恢复未完成，请查看文件记录与原因';
}
export function FileWorkflows({ store }: { store: IrisStore }) {
  const state = useIris(store);
  const [flow, setFlow] = useState<Flow>(null);
  const [step, setStep] = useState<'setup' | 'confirm'>('setup');
  const [format, setFormat] = useState<'copy' | 'xmp' | 'csv'>('copy');
  const [destination, setDestination] = useState('');
  const [scope, setScope] = useState('keep');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [manifest, setManifest] = useState<string | null>(null);
  const [restoreFailure, setRestoreFailure] = useState<{ id: string; reason: string; summary: string } | null>(null);
  const [exportPaths, setExportPaths] = useState<string[]>([]);
  const run = async (operation: () => Promise<void>) => {
    if (busy) return;
    setBusy(true); setError(null);
    try { await operation(); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
    finally { setBusy(false); }
  };
  const close = () => { if (!busy) setFlow(null); };
  const chooseDestination = async () => {
    if (!window.__TAURI__) throw new Error('请在桌面应用中选择保存位置');
    const selected = await (format === 'csv' ? selectExportCsv : selectProjectFolder)(window.__TAURI__.core);
    if (selected) setDestination(selected);
  };
  const exportFiles = async () => {
    if (!state.project) return;
    const report = format === 'copy'
      ? await store.client.exportCopy(state.project.id, destination, scope)
      : format === 'csv' ? await store.client.exportCsv(state.project.id, destination)
      : await store.client.exportXmp(state.project.id, scope, false);
    setExportPaths(report.paths);
    setMessage(`已导出 ${report.written} 项，跳过 ${report.skipped} 项`); setFlow(null);
  };
  const plan = state.quarantine;
  return <section className="stack"><h2>文件操作</h2><div className="row">
    <Button disabled={busy} onClick={() => { setStep('setup'); setFlow('export'); setError(null); }}><Icon name="export" />导出</Button>
    <Button disabled={busy} onClick={() => void run(async () => { await store.previewQuarantine(); setFlow('quarantine'); })}>隔离移除项</Button>
    <Button disabled={busy} onClick={() => { setFlow('restore'); setManifest(null); setError(null); }}>恢复</Button>
  </div>
    {message && <p role="status">{message}</p>}{error && !flow && !restoreFailure && <p role="alert">{error}</p>}
    {exportPaths.length > 0 && <details><summary>上次导出位置</summary>{exportPaths.map(path => <p className="filename" key={path}>{path}</p>)}</details>}
    {restoreFailure && flow !== 'restore' && <div role="alert"><p>{restoreFailure.summary}</p><details><summary>查看原因</summary><p>{restoreFailure.reason}</p></details><Button disabled={busy} onClick={() => { setManifest(restoreFailure.id); setError(null); setFlow('restore'); }}>重试恢复</Button></div>}
    {flow === 'export' && <Dialog title={step === 'setup' ? '导出照片' : '确认导出'} onClose={close}>
      {step === 'setup' ? <div className="stack"><label>导出方式<select disabled={busy} value={format} onChange={event => { setFormat(event.target.value as typeof format); setDestination(''); }}><option value="copy">复制照片</option><option value="xmp">XMP 标记</option><option value="csv">CSV 选片记录</option></select></label>
        {format === 'csv' ? <p>全部照片的路径与选片决定，包含不可用照片。</p> : <label>范围<select disabled={busy} value={scope} onChange={event => setScope(event.target.value)}><option value="keep">保留的照片</option><option value="reject">移除的照片</option><option value="all">全部照片</option></select></label>}
        {format !== 'xmp' && <div className="stack"><p className="filename">{destination || '尚未选择保存位置'}</p><Button disabled={busy} onClick={() => void run(chooseDestination)}>{format === 'csv' ? '选择 CSV 保存位置' : '选择目标文件夹'}</Button></div>}
        <div className="dialog-actions"><Button primary disabled={busy || (format !== 'xmp' && !destination)} onClick={() => setStep('confirm')}>下一步</Button></div>
      </div> : <><dl><dt>方式</dt><dd>{format === 'copy' ? '复制照片' : format === 'csv' ? 'CSV 选片记录' : 'XMP 标记'}</dd><dt>范围</dt><dd>{format === 'csv' || scope === 'all' ? '全部照片' : scope === 'keep' ? '保留的照片' : '移除的照片'}</dd><dt>位置</dt><dd className="filename">{format !== 'xmp' ? destination : state.project?.root}</dd><dt>同名文件</dt><dd>{format === 'csv' ? '已有文件时停止导出' : '跳过并保留现有文件'}</dd></dl>
        <div className="dialog-actions"><Button disabled={busy} onClick={() => setStep('setup')}>上一步</Button><Button primary disabled={busy} onClick={() => void run(exportFiles)}>{busy ? '正在导出' : '确认导出'}</Button></div></>}
      {error && <p role="alert">{error}</p>}
    </Dialog>}
    {flow === 'quarantine' && plan && <Dialog title="隔离标记移除的照片？" onClose={close}><p>共 {plan.items.length} 个文件，将移入隔离目录。</p><details><summary>查看文件</summary>{plan.items.map(item => <p className="filename" key={item.photo_id}>{item.source}</p>)}</details>
      {error && <p role="alert">{error}</p>}<div className="dialog-actions"><Button disabled={busy} onClick={close}>取消</Button><Button primary disabled={busy || !plan.items.length} onClick={() => void run(async () => { await store.commitQuarantine(plan.id); setMessage('已移入隔离目录'); setFlow(null); })}>{busy ? '正在隔离' : '确认隔离'}</Button></div>
    </Dialog>}
    {flow === 'restore' && <Dialog title={manifest ? '恢复这批照片？' : '隔离记录'} onClose={close}>
      {manifest ? <><p>将照片恢复到原位置，已有同名文件会阻止恢复。</p><details><summary>文件记录</summary>{state.quarantinePlans.find(item => item.id === manifest)?.items.map(item => <p className="filename" key={item.photo_id}>{item.source} · {recoveryStates[item.state] ?? item.state}</p>)}</details><div className="dialog-actions"><Button disabled={busy} onClick={() => setManifest(null)}>返回</Button><Button primary disabled={busy} onClick={() => void run(async () => { try { await store.restoreQuarantine(manifest); setRestoreFailure(null); setMessage('照片已恢复'); setFlow(null); } catch (reason) { const message = reason instanceof Error ? reason.message : String(reason); const status = store.getSnapshot().quarantinePlans.find(item => item.id === manifest)?.state; setRestoreFailure({ id: manifest, reason: message, summary: recoverySummary(message, status) }); throw reason; } })}>{busy ? '正在恢复' : '确认恢复'}</Button></div></>
      : <div className="stack">{state.quarantinePlans.length ? state.quarantinePlans.map(item => <div key={item.id} className="row"><span>{item.items.length} 个文件 · {recoveryStates[item.state] ?? item.state}</span><Button disabled={item.state === 'restored'} onClick={() => setManifest(item.id)}>恢复</Button></div>) : <p>暂无隔离记录</p>}</div>}
      {error && <div role="alert"><p>{restoreFailure?.id === manifest ? restoreFailure.summary : '恢复未完成'}</p><details><summary>查看原因</summary><p>{error}</p></details></div>}
    </Dialog>}
  </section>;
}
