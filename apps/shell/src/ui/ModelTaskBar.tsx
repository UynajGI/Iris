import { useEffect, useState } from 'react';
import type { IrisStore } from '../store.js';
import type { InstallProgress } from '../types.js';
import { Button } from './components.js';

const active = ['starting', 'downloading', 'importing', 'verifying', 'installing'];
const names: Record<string, string> = { starting: '准备中', downloading: '下载中', importing: '导入中', verifying: '校验中', installing: '安装中', complete: '已安装', failed: '安装失败', cancelled: '已取消' };
export function ModelTaskBar({ store, reveal, onSettings }: { store: IrisStore; reveal: number; onSettings(): void }) {
  const [progress, setProgress] = useState<InstallProgress | null>(null);
  const [hidden, setHidden] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { setHidden(false); }, [reveal]);
  useEffect(() => {
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const report = await store.client.modelInstallStatus();
        if (stopped) return;
        setProgress(report); setError(null);
        if (active.includes(report.state)) setHidden(false);
      } catch { /* Project connection status covers a disconnected daemon. */ }
      finally { if (!stopped) timer = setTimeout(() => void poll(), 1500); }
    };
    void poll();
    return () => { stopped = true; clearTimeout(timer); };
  }, [store]);
  if (!progress || progress.state === 'idle' || hidden) return null;
  const running = active.includes(progress.state);
  return <footer className="taskbar" aria-label="模型安装任务"><div className="row"><strong>模型安装</strong><span role="status">{names[progress.state] ?? '处理中'}</span>
    {running && <progress max={Math.max(1, progress.total_bytes)} value={progress.total_bytes ? progress.completed_bytes : undefined} aria-label="模型安装进度" />}
    <Button onClick={onSettings}>查看模型</Button>
    {running ? <Button onClick={() => void store.client.cancelModelInstall().catch(reason => setError(String(reason)))}>取消安装</Button> : <Button onClick={() => setHidden(true)}>关闭提示</Button>}
  </div>{(progress.error || error) && <details><summary>查看原因</summary><p>{error || progress.error}</p></details>}</footer>;
}
