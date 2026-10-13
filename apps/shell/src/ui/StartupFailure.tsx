import { useEffect, useState } from 'react';
import { Button, Dialog } from './components.js';
import { ErrorDetails } from './ErrorDetails.js';
import { HelpCenter } from './HelpCenter.js';
import { ShortcutSheet } from './ShortcutSheet.js';

/** Recovery UI must not depend on a daemon session or an initialized IrisStore. */
export function StartupFailure({ error, onRetry }: { error: string; onRetry(): Promise<void> }) {
  const [help, setHelp] = useState(false);
  const [shortcuts, setShortcuts] = useState(false);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (event.key === 'F1') {
        event.preventDefault();
        setHelp(true);
      }
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, []);
  return (
    <div className="app">
      {help ? (
        <HelpCenter onBack={() => setHelp(false)} onShortcuts={() => setShortcuts(true)} />
      ) : (
        <main className="startup-failure">
          <h1>照片服务未就绪</h1>
          <p>请检查安装文件或重试连接。</p>
          <ErrorDetails error={error} />
          <div className="row">
            <Button
              disabled={busy}
              onClick={() => {
                setBusy(true);
                void onRetry().finally(() => setBusy(false));
              }}
            >
              {busy ? '正在连接…' : '重试连接'}
            </Button>
            <Button onClick={() => setHelp(true)}>帮助与关于</Button>
          </div>
        </main>
      )}
      {shortcuts && (
        <Dialog title="键盘快捷键" onClose={() => setShortcuts(false)}>
          <ShortcutSheet />
        </Dialog>
      )}
    </div>
  );
}
