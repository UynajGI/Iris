import { useEffect, useState } from 'react';
import type { IrisStore } from '../store.js';
import { useIris } from '../react.js';
import { selectProjectFolder } from '../native-dialog.js';
import { Button, Dialog } from './components.js';

export function CacheSettings({ store }: { store: IrisStore }) {
  const state = useIris(store);
  const [confirmation, setConfirmation] = useState<'cleanup' | 'migrate' | null>(null);
  const [destination, setDestination] = useState('');
  const [busy, setBusy] = useState(false);
  const [automaticCleanup, setAutomaticCleanup] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const run = async (operation: () => Promise<void>) => {
    if (busy) return;
    setBusy(true); setError(null);
    try { await operation(); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
    finally { setBusy(false); }
  };
  useEffect(() => {
    if (state.project) void store.refreshCache().catch(reason => setError(String(reason)));
  }, [store, state.project?.id]);
  if (!state.project) return null;
  return <section className="settings-section"><h2>缓存</h2>
    {state.cache && <dl><dt>位置</dt><dd>{state.cache.root}</dd><dt>占用空间</dt><dd>{Math.ceil(state.cache.bytes / 1048576)} MiB · {state.cache.files} 个文件</dd></dl>}
    <div className="row"><Button disabled={busy} onClick={() => setConfirmation('cleanup')}>清理缓存</Button><Button disabled={busy} onClick={() => void run(async () => {
      if (!window.__TAURI__) throw new Error('请在桌面应用中选择文件夹');
      const selected = await selectProjectFolder(window.__TAURI__.core);
      if (selected) { setDestination(selected); setConfirmation('migrate'); }
    })}>迁移缓存</Button></div>
    {message && <p role="status">{message}</p>}{error && <p role="alert">{error}</p>}
    {state.cacheMigrations.filter(item => item.state !== 'cleaned').map(item => <div key={item.id} className="stack"><p className="muted filename">{item.source_root} → {item.destination_root}</p><Button disabled={busy} onClick={() => { setError(null); store.reviewCacheMigration(item.id); }}>检查旧缓存</Button></div>)}
    {confirmation && <Dialog title={confirmation === 'cleanup' ? '清理缓存？' : '迁移缓存？'} onClose={() => { if (!busy) setConfirmation(null); }}>
      <p className="filename">{confirmation === 'cleanup' ? '预览缓存将被移除，需要时重新生成。' : `目标文件夹：${destination}`}</p>
      {confirmation === 'migrate' && <p>复制并校验完成后切换位置，再清理本次迁移清单中的旧缓存。原照片保留。</p>}
      {error && <p role="alert">{error}</p>}<div className="dialog-actions"><Button disabled={busy} onClick={() => setConfirmation(null)}>取消</Button><Button primary disabled={busy} onClick={() => void run(async () => {
        if (confirmation === 'cleanup') { await store.client.cleanupCache(state.project!.id); await store.refreshCache(); setMessage('缓存已清理'); }
        else {
          const before = new Set(store.getSnapshot().cacheMigrations.map(item => item.id));
          await store.migrateCache(destination);
          setConfirmation(null);
          setMessage('已切换缓存位置，正在清理旧缓存');
          const latest = store.getSnapshot();
          const migrated = latest.cacheMigrations.filter(item => !before.has(item.id) && item.destination_root === latest.cache?.root);
          if (migrated.length !== 1) { setMessage('缓存位置已确认；没有唯一的新迁移清单，未清理旧文件'); return; }
          setAutomaticCleanup(true);
          store.reviewCacheMigration(migrated[0]!.id);
          try {
            await store.cleanupPreviousCache(migrated[0]!.id);
            setMessage('缓存已迁移，清单中的旧缓存已清理');
          } catch (reason) { setMessage('已切换缓存位置，旧缓存清理未完成'); throw reason; }
          finally { store.clearCacheMigrationReview(); setAutomaticCleanup(false); }
        }
        setConfirmation(null);
      })}>{busy ? '处理中' : '确认'}</Button></div>
    </Dialog>}
    {state.cacheMigration && !automaticCleanup && <Dialog title="清理已迁移的旧缓存？" onClose={() => { if (!busy) store.clearCacheMigrationReview(); }}><div className="stack"><p className="filename">{state.cacheMigration.source_root}</p><p>仅清理迁移清单内、已验证副本的旧文件。</p><details><summary>迁移详情</summary><p>{state.cacheMigration.files.length} 个文件</p>{state.cacheMigration.error && <p>{state.cacheMigration.error}</p>}</details></div>
      {error && <p role="alert">{error}</p>}<div className="dialog-actions"><Button disabled={busy} onClick={() => store.clearCacheMigrationReview()}>取消</Button><Button primary disabled={busy} onClick={() => void run(async () => { await store.cleanupPreviousCache(state.cacheMigration!.id); store.clearCacheMigrationReview(); setMessage('清单中的旧缓存已清理'); })}>确认清理</Button></div>
    </Dialog>}
  </section>;
}
