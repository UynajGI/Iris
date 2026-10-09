import { createRoot } from 'react-dom/client';
import { useEffect, type ReactNode } from 'react';
import { App } from './ui/App.js';
import { AppBoundary } from './ui/AppBoundary.js';
import { connectDesktop } from './desktop.js';

const element = document.getElementById('root');
if (!element) throw new Error('Application root is missing');
const root = createRoot(element);
function FirstPaint({ children }: { children: ReactNode }) {
  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      void window.__TAURI__?.core.invoke('frontend_ready').catch(error => {
        window.dispatchEvent(new CustomEvent('iris:error', { detail: String(error) }));
      });
    });
    return () => cancelAnimationFrame(frame);
  }, []);
  return children;
}
let readyApplication: Window['iris'];
function renderApplication(failure?: string) {
  root.render(<FirstPaint><AppBoundary>
    {failure && <div className="error-bar" role="alert"><p>照片服务连接未恢复</p><details><summary>查看原因</summary><p>{failure}</p></details><button className="button" onClick={() => { if (window.__TAURI__) void connectDesktop(window.__TAURI__).catch(error => renderApplication(String(error))); }}>重试连接</button></div>}
    {readyApplication ? <App key="application" store={readyApplication.store} commands={readyApplication.commands} /> : !failure && <div className="app"><main className="empty" role="status"><h1 className="brand">伊人</h1><p>正在连接照片服务</p></main></div>}
  </AppBoundary></FirstPaint>);
}
renderApplication();
window.addEventListener('iris:ready', () => { readyApplication = window.iris; renderApplication(); });
window.addEventListener('iris:error', event => {
  const message = String((event as CustomEvent<unknown>).detail);
  renderApplication(message);
});
