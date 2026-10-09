import { Component, type ReactNode } from 'react';
import { Button } from './components.js';

/** Rendering failures stay local; no photographs or diagnostics leave the app. */
export class AppBoundary extends Component<{ children: ReactNode }, { error: string | null }> {
  state: { error: string | null } = { error: null };
  static getDerivedStateFromError(error: unknown) {
    return { error: error instanceof Error ? error.message : String(error) };
  }
  render() {
    if (this.state.error !== null) return <main className="page"><div className="page-content stack" role="alert">
      <h1>界面未能显示</h1><p>重新打开界面会放弃未保存的设置。</p>
      <div className="row"><Button primary onClick={() => this.setState({ error: null })}>重新打开界面</Button><Button onClick={() => void window.__TAURI__?.core.invoke('confirm_close').catch(() => {})}>关闭应用</Button></div>
      <details><summary>查看原因</summary><p>{this.state.error}</p></details>
    </div></main>;
    return this.props.children;
  }
}
