import { useState } from 'react';
import { copyText, errorReport, openSupportLink } from '../support.js';
import { Button } from './components.js';

export function ErrorDetails({ error }: { error: string }) {
  const [notice, setNotice] = useState('');
  const [busy, setBusy] = useState(false);
  const act = (work: () => Promise<void>, success = '') => {
    if (busy) return;
    setBusy(true);
    setNotice('');
    void work()
      .then(() => {
        if (success) setNotice(success);
      })
      .catch(() => setNotice('操作未完成，请手动复制或通过帮助页反馈'))
      .finally(() => setBusy(false));
  };
  return (
    <div className="error-details">
      <details>
        <summary>查看详细原因</summary>
        <p className="error-reason">{error}</p>
      </details>
      <div className="row">
        <Button
          className="quiet"
          disabled={busy}
          onClick={() => act(() => copyText(errorReport(error)), '已复制脱敏错误摘要')}
        >
          复制错误摘要
        </Button>
        {typeof window !== 'undefined' && window.__TAURI__ && (
          <Button
            className="quiet"
            disabled={busy}
            onClick={() =>
              act(async () => {
                const saved = await window.__TAURI__!.core.invoke<boolean>('export_diagnostics');
                setNotice(saved ? '诊断信息已保存' : '已取消导出');
              })
            }
          >
            导出诊断信息
          </Button>
        )}
        <Button
          className="quiet"
          disabled={busy}
          onClick={() => act(() => openSupportLink('issues'))}
        >
          反馈问题
        </Button>
        {notice && <span role="status">{notice}</span>}
      </div>
    </div>
  );
}
