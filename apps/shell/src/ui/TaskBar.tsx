import { useEffect, useState } from 'react';
import type { IrisStore } from '../store.js';
import { useIris } from '../react.js';
import { Button } from './components.js';
import { ExecutionDetails } from './ExecutionDetails.js';

const states: Record<string,string> = { running:'进行中', paused:'已暂停', cancelled:'已停止', completed:'已完成', failed:'未全部完成' };
export function TaskBar({ store, onSettings, reveal = 0 }: { store: IrisStore; onSettings(): void; reveal?: number }) {
  const state = useIris(store);
  const [dismissed, setDismissed] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [requesting, setRequesting] = useState(false);
  useEffect(() => { setDismissed(''); }, [reveal]);
  const progress = state.progress;
  if (!progress || progress.kind === 'none' || progress.state === 'idle') return null;
  const key = `${state.project?.id}:${progress.id || JSON.stringify(progress)}`;
  const running = ['running','paused'].includes(progress.state);
  if (!running && key === dismissed) return null;
  const run = async (operation: () => Promise<void>) => {
    if (requesting) return;
    setRequesting(true); setError(null);
    try { await operation(); } catch (reason) { setError(String(reason)); } finally { setRequesting(false); }
  };
  return <footer className="taskbar" aria-label="任务进度"><div className="row">
    <strong>{progress.kind === 'scan' ? '扫描' : '分析'}</strong><span role="status">{states[progress.state] ?? '等待中'}</span>
    <progress max={Math.max(1,progress.total)} value={progress.kind === 'scan' && running ? undefined : progress.total ? progress.completed : undefined} aria-label="任务进度" />
    <span>{progress.kind === 'scan' ? `已找到 ${progress.found_photos ?? 0} 张` : progress.failed_photo_ids?.length ? `${Math.max(0, progress.completed - progress.failed_photo_ids.length)} 张完成 · ${progress.failed_photo_ids.length} 张失败` : progress.total ? `${progress.completed} / ${progress.total}` : '准备中'}</span>
    {running ? <Button disabled={requesting} onClick={() => void run(() => store.run('cancel'))}>{requesting ? '正在停止' : '停止'}</Button> : <>
      {progress.kind === 'scan' && !!progress.failed_scan_paths?.length && <Button disabled={requesting} onClick={() => void run(() => store.retryScan())}>重试扫描失败项</Button>}
      {progress.kind === 'analysis' && !!progress.failed_photo_ids?.length && <Button disabled={requesting} onClick={() => void run(() => store.retryFailed())}>重试失败的 {progress.failed_photo_ids.length} 张</Button>}
      {progress.kind === 'analysis' && progress.state === 'failed' && !progress.failed_photo_ids?.length && <><Button disabled={requesting} onClick={() => void run(() => store.run('analyze'))}>重试分析</Button><Button onClick={onSettings}>检查设置</Button></>}
      {progress.kind === 'analysis' && progress.state === 'cancelled' && <Button disabled={requesting} onClick={() => void run(() => store.run('analyze'))}>继续分析</Button>}
      <Button onClick={() => setDismissed(key)}>关闭提示</Button>
    </>}
  </div>
    {progress.root_unavailable && <p role="alert">照片目录无法访问，请恢复连接后重试扫描。</p>}
    {progress.errors.length > 0 && <details><summary>{progress.errors.length} 项问题</summary>{progress.errors.map((message,index) => <p key={index}>{message}</p>)}</details>}
    <ExecutionDetails progress={progress} />
    {error && <p role="alert">{error}</p>}
  </footer>;
}
