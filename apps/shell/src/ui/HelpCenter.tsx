import { useEffect, useState } from 'react';
import { APP_VERSION } from '../generated/version.js';
import { copyText, openSupportLink, versionText, type SupportInfo } from '../support.js';
import { Button } from './components.js';
import { ErrorDetails } from './ErrorDetails.js';
import { helpTopics } from './help-content.js';
import { HelpDocument } from './HelpDocument.js';
import type { UpdateStatus } from '../updater.js';

export function HelpCenter({ onBack, onShortcuts }: { onBack(): void; onShortcuts(): void }) {
  const [info, setInfo] = useState<SupportInfo>();
  const [update, setUpdate] = useState<UpdateStatus | null>(null);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let active = true;
    const bridge = window.__TAURI__?.core;
    if (bridge)
      void bridge
        .invoke<SupportInfo>('support_info')
        .then((value) => {
          if (active) setInfo(value);
        })
        .catch(() => {
          if (active) setError('无法读取桌面版本信息，请重新打开帮助页');
        });
    const updater = window.iris?.updater;
    const refresh = () => {
      if (active && updater) setUpdate(updater.getSnapshot().status);
    };
    refresh();
    const unsubscribe = updater?.subscribe(refresh);
    return () => {
      active = false;
      unsubscribe?.();
    };
  }, []);
  const act = (operation: () => Promise<unknown>, message = '') => {
    if (busy) return;
    setBusy(true);
    setError('');
    setNotice('');
    void operation()
      .then(() => {
        if (message) setNotice(message);
      })
      .catch((reason) => {
        setError(reason instanceof Error ? reason.message : String(reason));
      })
      .finally(() => setBusy(false));
  };
  const exportReport = async () => {
    const saved = await window.__TAURI__?.core.invoke<boolean>('export_diagnostics');
    setNotice(saved ? '诊断信息已保存，可检查后自行发送' : '已取消导出');
  };
  return (
    <main className="page help-page">
      <div className="page-content">
        <header className="page-heading">
          <h1>帮助与关于</h1>
          <Button onClick={onBack}>返回</Button>
        </header>
        <div className="help-layout">
          <nav aria-label="帮助分类" className="settings-nav">
            <a href="#help-about">关于伊人</a>
            <a href="#help-guide">使用帮助</a>
            <a href="#help-diagnostics">日志与反馈</a>
          </nav>
          <div className="help-body">
            <section id="help-about" className="settings-section">
              <h2>伊人 / Iris</h2>
              <p>本地智能选片软件</p>
              <dl className="support-facts">
                <dt>版本</dt>
                <dd>{info?.version ?? APP_VERSION}</dd>
                <dt>构建</dt>
                <dd>
                  {info?.build ?? '浏览器预览'}
                  {info?.debug ? ' · 开发构建' : ''}
                </dd>
                {info && (
                  <>
                    <dt>运行环境</dt>
                    <dd>
                      {info.platform} · {info.architecture} · {info.distribution}
                    </dd>
                  </>
                )}
              </dl>
              {info &&
                (info.version !== APP_VERSION ||
                  (info.daemon_version && info.daemon_version !== info.version)) && (
                  <p role="alert">应用组件版本不同，请重新安装完整的软件包。</p>
                )}
              <div className="row">
                <Button
                  disabled={busy}
                  onClick={() => act(() => copyText(versionText(info)), '已复制版本信息')}
                >
                  复制版本信息
                </Button>
                <Button disabled={busy} onClick={() => act(() => openSupportLink('homepage'))}>
                  项目主页
                </Button>
              </div>
              <h3>版本与更新</h3>
              <p>
                {update?.state === 'not_configured'
                  ? '此安装包未启用自动更新，可在下载页面查看已发布版本。'
                  : update?.state === 'available'
                    ? `发现版本 ${update.update?.version ?? ''}，请在下载页面查看说明。`
                    : '可在下载页面查看已发布版本。本页不会自动检查、下载或安装更新。'}
              </p>
              <Button disabled={busy} onClick={() => act(() => openSupportLink('releases'))}>
                打开下载页面
              </Button>
              <HelpDocument name="release-notes" title="当前版本说明" />
              <h3>开源许可</h3>
              <p>Iris 使用 GPL-3.0-or-later；随附组件和模型遵循各自许可。</p>
              <HelpDocument name="LICENSE" title="软件许可证" />
              <HelpDocument name="THIRD_PARTY_NOTICES" title="第三方声明" />
            </section>
            <section id="help-guide" className="settings-section">
              <h2>使用帮助</h2>
              <Button onClick={onShortcuts}>查看键盘快捷键</Button>
              {helpTopics.map((topic) => (
                <details className="help-topic" key={topic.title}>
                  <summary>{topic.title}</summary>
                  <p>{topic.body}</p>
                </details>
              ))}
            </section>
            <section id="help-diagnostics" className="settings-section">
              <h2>日志与反馈</h2>
              <p>
                日志只保存关键事件，最多约 1
                MiB。诊断文件不含照片、路径或原始错误文字，不会自动发送。
              </p>
              {info && (
                <dl className="support-facts">
                  <dt>数据目录</dt>
                  <dd className="filename">{info.data_directory}</dd>
                  <dt>日志目录</dt>
                  <dd className="filename">{info.log_directory}</dd>
                </dl>
              )}
              {info && !info.logs_available && (
                <p role="alert">日志目录不可写，请检查磁盘空间与权限。</p>
              )}
              {!info && <p className="muted">日志与诊断导出需要在桌面应用中使用。</p>}
              <div className="row">
                <Button
                  disabled={busy || !info?.logs_available}
                  onClick={() => act(() => openSupportLink('logs'))}
                >
                  打开日志文件夹
                </Button>
                <Button disabled={busy || !info?.logs_available} onClick={() => act(exportReport)}>
                  导出诊断信息
                </Button>
                <Button disabled={busy} onClick={() => act(() => openSupportLink('issues'))}>
                  反馈问题
                </Button>
              </div>
              <p className="muted">
                反馈时请写明操作步骤、预期结果与实际表现，附上版本信息；不必附带照片。
              </p>
            </section>
            {busy && <p role="status">正在处理…</p>}
            {notice && <p role="status">{notice}</p>}
            {error && (
              <div role="alert">
                <p>操作未完成</p>
                <ErrorDetails error={error} />
              </div>
            )}
          </div>
        </div>
      </div>
    </main>
  );
}
