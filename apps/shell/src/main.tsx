import { createRoot } from 'react-dom/client';
import { useEffect, type ReactNode } from 'react';
import { App } from './ui/App.js';
import { AppBoundary } from './ui/AppBoundary.js';
import { connectDesktop } from './desktop.js';
import { ErrorDetails } from './ui/ErrorDetails.js';
import { recordDiagnostic } from './support.js';
import { StartupFailure } from './ui/StartupFailure.js';
window.addEventListener('error', () => recordDiagnostic('frontend_failed'));
window.addEventListener('unhandledrejection', () => recordDiagnostic('frontend_failed'));

const element = document.getElementById('root');
if (!element) throw new Error('Application root is missing');
const root = createRoot(element);
function FirstPaint({ children }: { children: ReactNode }) {
  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      void window.__TAURI__?.core.invoke('frontend_ready').catch((error) => {
        window.dispatchEvent(new CustomEvent('iris:error', { detail: String(error) }));
      });
    });
    return () => cancelAnimationFrame(frame);
  }, []);
  return children;
}
let readyApplication: Window['iris'];
async function retryConnection() {
  if (window.__TAURI__)
    await connectDesktop(window.__TAURI__).catch((error) => renderApplication(String(error)));
}
function renderApplication(failure?: string) {
  root.render(
    <FirstPaint>
      <AppBoundary>
        {failure && readyApplication && (
          <div className="error-bar" role="alert">
            <span>照片服务未就绪，请检查安装文件或重试连接</span>
            <ErrorDetails error={failure} />
            <span className="spacer" />
            <button className="button" onClick={() => void retryConnection()}>
              重试连接
            </button>
          </div>
        )}
        {readyApplication ? (
          <App
            key="application"
            store={readyApplication.store}
            commands={readyApplication.commands}
          />
        ) : failure ? (
          <StartupFailure error={failure} onRetry={retryConnection} />
        ) : (
          <div className="app">
            <main className="splash" role="status">
              <h1 className="brand">伊人</h1>
              <p className="muted">正在连接照片服务</p>
              <span className="splash-line" aria-hidden="true" />
            </main>
          </div>
        )}
      </AppBoundary>
    </FirstPaint>
  );
}
renderApplication();
window.addEventListener('iris:ready', () => {
  readyApplication = window.iris;
  renderApplication();
});
window.addEventListener('iris:error', (event) => {
  const message = String((event as CustomEvent<unknown>).detail);
  renderApplication(message);
});
