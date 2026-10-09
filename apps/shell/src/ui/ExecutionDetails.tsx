import type { Progress } from '../types.js';

export function ExecutionDetails({ progress }: { progress: Progress | null }) {
  const current = !!progress?.execution?.length;
  const entries = current ? progress?.execution : progress?.previous_execution;
  if (!entries?.length) return null;
  return <details><summary>{current ? '本次分析的计算设备' : '上次分析的计算设备'}</summary><div className="stack">{entries.map(entry => {
    const fallback = entry.warnings.some(note => /CPU fallback|retrying on CPU/.test(note));
    const registered = entry.warnings.some(note => note.includes('DirectML registered'));
    const status = !entry.completed_items ? '尚未完成推理' : fallback ? '包含 CPU 回退' : entry.selected_provider === 'cpu' ? 'CPU 推理已完成' : registered ? 'GPU 会话已启用，允许 CPU 算子' : '尚无执行设备报告';
    return <section key={entry.phase}><h3>{entry.phase === 'quality' ? '照片质量' : '相似度计算'}</h3>
      <p>选择：{entry.selected_provider === 'cpu' ? 'CPU' : entry.device_name ?? `GPU ${entry.device_id ?? ''}`}</p>
      <p>{status} · 完成 {entry.completed_items} 张</p>
      {entry.warnings.length > 0 && <details><summary>运行详情</summary>{entry.warnings.map((note,index) => <p key={index}>{note}</p>)}</details>}
    </section>;
  })}</div></details>;
}
