import { useEffect, useState } from 'react';
import type { IrisStore } from '../store.js';
import { useIris } from '../react.js';
import { Button } from './components.js';
import { ExecutionDetails } from './ExecutionDetails.js';
import { Icon } from './Icon.js';

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
  const failures = progress.failed_photo_ids?.length ?? 0;
  const tone = running ? 'running' : progress.state === 'cancelled' ? 'idle' : progress.state === 'completed' && !failures && !progress.errors.length ? 'done' : 'attention';
  const hasDetails = progress.root_unavailable || progress.errors.length > 0 || !!progress.execution?.length || !!progress.previous_execution?.length || !!error;
  return <footer className={`taskbar taskbar-${tone}`} aria-label="任务进度"><div className="taskbar-row">
    <span className="task-state-icon" aria-hidden="true">{running ? <span className="activity-dot" /> : <Icon name={tone === 'done' ? 'done' : tone === 'attention' ? 'warning' : 'stop'} />}</span>
    <strong>{progress.kind === 'scan' ? '扫描' : '分析'}</strong><span role="status" className="muted">{states[progress.state] ?? '等待中'}</span>
    <progress max={Math.max(1,progress.total)} value={progress.kind === 'scan' && running ? undefined : progress.total ? progress.completed : undefined} aria-label="任务进度" />
    <span className="tabular task-count">{progress.kind === 'scan' ? `已找到 ${progress.found_photos ?? 0} 张` : failures ? `${Math.max(0, progress.completed - failures)} 张完成 · ${failures} 张失败` : progress.total ? `${progress.completed} / ${progress.total}` : '准备中'}</span>
    <span className="spacer" />
    {running ? <Button disabled={requesting} onClick={() => void run(() => store.run('cancel'))}><Icon name="stop" />{requesting ? '正在停止' : '停止'}</Button> : <>
      {progress.kind === 'scan' && !!progress.failed_scan_paths?.length && <Button disabled={requesting} onClick={() => void run(() => store.retryScan())}><Icon name="refresh" />重试扫描失败项</Button>}
      {progress.kind === 'analysis' && failures > 0 && <Button disabled={requesting} onClick={() => void run(() => store.retryFailed())}><Icon name="refresh" />重试失败的 {failures} 张</Button>}
      {progress.kind === 'analysis' && progress.state === 'failed' && !failures && <><Button disabled={requesting} onClick={() => void run(() => store.run('analyze'))}><Icon name="refresh" />重试分析</Button><Button className="quiet" onClick={onSettings}>检查设置</Button></>}
      {progress.kind === 'analysis' && progress.state === 'cancelled' && <Button disabled={requesting} onClick={() => void run(() => store.run('analyze'))}><Icon name="play" />继续分析</Button>}
      <Button className="quiet icon-only" aria-label="关闭提示" title="关闭提示" onClick={() => setDismissed(key)}><Icon name="close" /></Button>
    </>}
  </div>
    {hasDetails && <div className="taskbar-details">
      {progress.root_unavailable && <p role="alert">照片目录无法访问，请恢复连接后重试扫描。</p>}
      {progress.errors.length > 0 && <details><summary>{progress.errors.length} 项问题</summary>{progress.errors.map((message,index) => <p key={index}>{message}</p>)}</details>}
      <ExecutionDetails progress={progress} />
      {error && <p role="alert">{error}</p>}
    </div>}
  </footer>;
}
