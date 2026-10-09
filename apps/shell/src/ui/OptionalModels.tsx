import { useEffect, useState } from 'react';
import type { IrisStore } from '../store.js';
import type { InstallProgress, OptionalModel, Settings } from '../types.js';
import { selectProjectFolder } from '../native-dialog.js';
import { Button, Dialog } from './components.js';
import dinoLicense from './licenses/DINOv3.txt';

const activeStates = ['starting','downloading','importing','verifying','installing'];
const stateNames: Record<string,string> = { starting:'准备中', downloading:'下载中', importing:'导入中', verifying:'校验中', installing:'安装中', complete:'安装完成', failed:'安装失败', cancelled:'已取消' };
export function OptionalModels({ store, draft, onChange }: { store: IrisStore; draft?: Settings; onChange?(settings: Settings): void }) {
  const [catalog, setCatalog] = useState<OptionalModel[]>([]);
  const [progress, setProgress] = useState<InstallProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pollError, setPollError] = useState<string | null>(null);
  const [requesting, setRequesting] = useState(false);
  const [confirmModel, setConfirmModel] = useState<{ model: OptionalModel; offline: boolean } | null>(null);
  useEffect(() => {
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    let lastState = '';
    setPollError(null);
    const poll = async () => {
      try {
        const current = await store.client.modelInstallStatus();
        if (disposed) return;
        setProgress(current);
        if (!lastState || (current.state === 'complete' && lastState !== current.state)) {
          const models = await store.client.optionalModels();
          if (!disposed) setCatalog(models);
        }
        lastState = current.state;
        if (!disposed) setPollError(null);
      } catch (reason) { if (!disposed) setPollError(String(reason)); }
      finally { if (!disposed) timer = setTimeout(() => void poll(), 1000); }
    };
    void poll();
    return () => { disposed = true; clearTimeout(timer); };
  }, [store]);
  const running = requesting || !!progress && activeStates.includes(progress.state);
  const install = async (model: OptionalModel, offline: boolean) => {
    if (running) return;
    setRequesting(true); setError(null);
    try {
      let source_folder: string | undefined;
      if (offline) {
        if (!window.__TAURI__) throw new Error('请在桌面应用中选择模型文件夹');
        const folder = await selectProjectFolder(window.__TAURI__.core);
        if (!folder) return;
        source_folder = folder;
      }
      setProgress(await store.client.installModel({ model: model.id, ...(source_folder ? { source_folder } : {}) }));
    } catch (reason) { setError(String(reason)); }
    finally { setRequesting(false); }
  };
  return <section className="settings-section"><h2>可选模型</h2>
    {catalog.map(model => <div className="stack" key={model.id}><div className="row"><h3>{model.title}</h3><span className="muted">{model.state === 'available' ? '已安装' : model.state === 'invalid' ? '校验未通过' : '未安装'}</span></div>
      {draft && onChange && <label>在当前项目启用<input type="checkbox" checked={draft.embedding_provider === 'dinov3_vits16'} disabled={model.state !== 'available' || running} onChange={event => onChange({ ...draft, embedding_provider: event.target.checked ? 'dinov3_vits16' : 'none', embedding_model_sha256: model.sha256, semantic_similarity_threshold: draft.semantic_similarity_threshold ?? 0.85 })} /></label>}
      {model.state !== 'available' && <div className="row"><Button disabled={running} onClick={() => setConfirmModel({ model, offline: false })}>{model.state === 'invalid' ? '重新安装' : '下载模型'} · {Math.ceil(model.bytes / 1048576)} MiB</Button><Button disabled={running} onClick={() => setConfirmModel({ model, offline: true })}>离线导入</Button></div>}
      <details><summary>来源与许可</summary><p className="muted">DINOv3 ViT-S/16，由 onnx-community 转换；适用 {model.license}，许可文件随模型保存。</p><p className="muted">离线文件夹需包含 dinov3_vits16.onnx 和 LICENSE-DINOv3.md。</p></details>
    </div>)}
    {progress && progress.state !== 'idle' && <div role="status" className="stack"><div className="row"><span>{stateNames[progress.state] ?? progress.state}</span>{running && <Button onClick={() => void store.client.cancelModelInstall().catch(reason => setError(String(reason)))}>取消</Button>}</div>
      {running && <progress max={Math.max(1,progress.total_bytes)} value={progress.completed_bytes} aria-label="模型安装进度" />}
      {progress.state === 'complete' && draft?.embedding_provider !== 'dinov3_vits16' && <p className="muted">模型已安装，勾选并保存后启用。</p>}
      {progress.error && <details><summary>查看原因</summary><p>{progress.error}</p></details>}
    </div>}
    {error && <p role="alert">{error}</p>}
    {pollError && <p role="alert">{pollError}</p>}
    {confirmModel && <Dialog title={confirmModel.offline ? '导入模型' : '下载模型'} onClose={() => setConfirmModel(null)}><div className="stack"><h3>{confirmModel.model.title}</h3><p>{Math.ceil(confirmModel.model.bytes / 1048576)} MiB · {confirmModel.model.license}</p><details><summary>查看模型许可</summary><pre className="license-text">{dinoLicense}</pre></details><p className="muted">安装后需手动勾选并保存，才在当前项目使用。</p></div><div className="dialog-actions"><Button onClick={() => setConfirmModel(null)}>取消</Button><Button primary onClick={() => { const selection = confirmModel; setConfirmModel(null); void install(selection.model, selection.offline); }}>同意许可并继续</Button></div></Dialog>}
  </section>;
}
