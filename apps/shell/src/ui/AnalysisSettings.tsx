import { useEffect, useRef, useState } from 'react';
import type { IrisStore } from '../store.js';
import type { GpuDevices, Settings } from '../types.js';
import { useIris } from '../react.js';
import { Button, IntegerControl } from './components.js';
import { OptionalModels } from './OptionalModels.js';
import { ExecutionDetails } from './ExecutionDetails.js';

const weights = [
  ['eyes_weight', '眼部状态'], ['sharpness_weight', '清晰度'], ['face_weight', '面部质量'],
  ['exposure_weight', '曝光'], ['smile_weight', '表情'],
] as const;

export function AnalysisSettings({ store, onDirty, onSaveReady }: { store: IrisStore; onDirty(value: boolean): void; onSaveReady(save: (() => Promise<boolean>) | null): void }) {
  const state = useIris(store);
  const [draft, setDraft] = useState<Settings | null>(state.settings ? structuredClone(state.settings) : null);
  const baseline = useRef({ projectId: state.project?.id, settings: state.settings });
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [devices, setDevices] = useState<GpuDevices | null>(null);
  const [deviceError, setDeviceError] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    void store.client.gpuDevices().then(value => { if (active) setDevices(value); }).catch(error => { if (active) setDeviceError(String(error)); });
    return () => { active = false; };
  }, [store]);
  const dirty = JSON.stringify(draft) !== JSON.stringify(baseline.current.settings);
  useEffect(() => { onDirty(dirty); }, [dirty, onDirty]);
  useEffect(() => () => onDirty(false), [onDirty]);
  useEffect(() => {
    // A reconnect refreshes server state; it must not replace this project's unsaved draft.
    if (baseline.current.projectId === state.project?.id && JSON.stringify(draft) !== JSON.stringify(baseline.current.settings)) return;
    baseline.current = { projectId: state.project?.id, settings: state.settings };
    setDraft(state.settings ? structuredClone(state.settings) : null);
  }, [state.project?.id, state.settings]);
  useEffect(() => {
    if (!dirty) return;
    const handler = (event: BeforeUnloadEvent) => { event.preventDefault(); };
    window.addEventListener('beforeunload', handler);
    return () => window.removeEventListener('beforeunload', handler);
  }, [dirty]);
  const projectId = state.project?.id;
  const update = <K extends keyof Settings>(key: K, value: Settings[K]) => { if (draft) setDraft({ ...draft, [key]: value }); setMessage(null); };
  const save = async () => {
    if (!draft || projectId === undefined || saving) return false;
    setSaving(true); setMessage(null);
    try {
      await store.saveSettings(draft);
      const saved = store.getSnapshot().settings;
      baseline.current = { projectId, settings: saved };
      setDraft(saved ? structuredClone(saved) : null);
      setMessage(store.getSnapshot().project?.pending_analysis ? '已保存 · 分析与分组待更新' : '已保存 · 分组待更新');
      return true;
    }
    catch (error) { setMessage(error instanceof Error ? error.message : String(error)); return false; }
    finally { setSaving(false); }
  };
  const defaults = async () => {
    if (!draft) return;
    setSaving(true);
    try {
      const contract = await store.client.contract() as { components?: { schemas?: { AnalysisSettings?: { properties?: Record<string, { default?: unknown }> } } } };
      const properties = contract.components?.schemas?.AnalysisSettings?.properties;
      if (!properties) throw new Error('无法读取默认参数，请重试');
      const values = Object.fromEntries(Object.entries(properties).filter(([, property]) => Object.hasOwn(property, 'default')).map(([name, property]) => [name, property.default]));
      setDraft({ ...draft, ...values, execution_provider: 'auto', directml_device_id: null }); setMessage('默认值已载入，保存后生效');
    } catch (error) { setMessage(String(error)); }
    finally { setSaving(false); }
  };
  useEffect(() => { onSaveReady(save); return () => onSaveReady(null); }, [onSaveReady, draft, projectId, saving, store]);
  if (!state.project || !draft) return <><section className="settings-section"><h2>分析</h2><p className="muted">打开项目后可调整分析设置</p></section><OptionalModels store={store} /></>;
  const selectedDevice = draft.execution_provider === 'directml' ? `gpu:${draft.directml_device_id ?? 0}` : draft.execution_provider ?? 'cpu';
  const adapters = devices?.adapters.filter(adapter => !adapter.is_software && !adapter.is_remote) ?? [];
  return <><OptionalModels store={store} draft={draft} onChange={setDraft} /><section className="settings-section"><h2>分析参数</h2>
    <label>计算设备<select value={selectedDevice} onChange={event => {
      const value = event.target.value;
      setDraft({ ...draft, execution_provider: value.startsWith('gpu:') ? 'directml' : value as 'auto' | 'cpu', directml_device_id: value.startsWith('gpu:') ? Number(value.slice(4)) : null });
    }}><option value="auto">Auto</option><option value="cpu">CPU</option>{adapters.map(adapter => <option key={adapter.device_id} value={`gpu:${adapter.device_id}`}>{adapter.name}{adapters.some(other => other.device_id !== adapter.device_id && other.name === adapter.name) ? ` · GPU ${adapter.device_id}` : ''}</option>)}{selectedDevice.startsWith('gpu:') && !adapters.some(adapter => `gpu:${adapter.device_id}` === selectedDevice) && <option value={selectedDevice}>原选 GPU 当前不可用</option>}</select></label>
    {draft.execution_provider === 'auto' && <p className="muted">优先可用的独立显卡，不可用时使用 CPU。</p>}
    {deviceError && <p role="status">显卡列表暂不可用。{deviceError}</p>}
    <ExecutionDetails progress={state.progress} />
    <IntegerControl label="建议保留阈值" min={Math.floor(draft.reject_threshold) + 1} value={draft.recommend_threshold} onChange={value => update('recommend_threshold', value)} />
    <IntegerControl label="建议移除阈值" max={Math.ceil(draft.recommend_threshold) - 1} value={draft.reject_threshold} onChange={value => update('reject_threshold', value)} />
    <IntegerControl label="人脸置信度" min={10} max={99} value={draft.face_confidence * 100} onChange={value => update('face_confidence', value / 100)} />
    <IntegerControl label="最多检测人脸" value={draft.max_faces} min={1} max={10} onChange={value => update('max_faces', value)} />
    <label>自然图像质量评估<input type="checkbox" checked={draft.enable_niqe} onChange={event => update('enable_niqe', event.target.checked)} /></label>
    {draft.embedding_provider !== 'none' && <IntegerControl label="相似度严格程度" min={1} value={(draft.semantic_similarity_threshold ?? 0.85) * 100} onChange={value => update('semantic_similarity_threshold', value / 100)} />}
    <details><summary>高级权重</summary><div className="stack">{weights.map(([key, label]) => <IntegerControl key={key} label={label} slider={false} value={draft[key]} onChange={value => update(key, value)} />)}<IntegerControl label="自然图像质量权重" slider={false} value={draft.niqe_weight * 100} onChange={value => update('niqe_weight', value / 100)} /></div></details>
    <div className="row"><Button primary disabled={!dirty || saving} onClick={() => void save()}>{saving ? '保存中' : '保存设置'}</Button><Button disabled={saving} onClick={() => void defaults()}>恢复默认值</Button></div>
    {message && <p role="status">{message}</p>}
    {!dirty && state.reanalysisRequired && <Button disabled={saving || !!state.progress?.root_unavailable || ['running', 'paused'].includes(state.progress?.state ?? '')} onClick={() => void store.run('analyze').catch(error => setMessage(String(error)))}>更新分析与分组</Button>}
  </section></>;
}
