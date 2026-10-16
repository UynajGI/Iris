import { useCallback, useEffect, useRef, useState } from 'react';
import type { CommandSystem } from '../commands.js';
import { selectProjectFolder } from '../native-dialog.js';
import { useIris } from '../react.js';
import type { IrisStore } from '../store.js';
import type { Project } from '../types.js';
import { AnalysisSettings } from './AnalysisSettings.js';
import { CacheSettings } from './CacheSettings.js';
import { CloseBehaviorSetting, useDesktopWindow, WindowCloseDialog } from './DesktopWindow.js';
import { Icon } from './Icon.js';
import { ModelTaskBar } from './ModelTaskBar.js';
import { PhotoWorkspace } from './PhotoWorkspace.js';
import { ShortcutSheet } from './ShortcutSheet.js';
import { TaskBar } from './TaskBar.js';
import { HelpCenter } from './HelpCenter.js';
import { ErrorDetails } from './ErrorDetails.js';
import { recordDiagnostic } from '../support.js';
import './app.css';
import './help.css';
import { Button, Dialog } from './components.js';
import { loadAppearance, saveAppearance, type Appearance } from './preferences.js';
import { usePhotoWorkspace } from './usePhotoWorkspace.js';
import { WorkflowNav, type AppPage } from './WorkflowNav.js';
import { EditingWorkspace } from './EditingWorkspace.js';
import { RenderedExportPanel } from './color/RenderedExportPanel.js';
import { FileWorkflows } from './FileWorkflows.js';

export function App({ store, commands }: { store: IrisStore; commands: CommandSystem }) {
  const state = useIris(store);
  const [page, setPage] = useState<AppPage>(() => {
    if (!state.project) return 'projects';
    try {
      const saved = sessionStorage.getItem('iris:review-page');
      if (
        document.querySelector('script[src="/__review/bridge.js"]') &&
        ['photos', 'color', 'retouch', 'export'].includes(saved ?? '')
      )
        return saved as AppPage;
    } catch {
      /* Optional review preferences. */
    }
    return 'photos';
  });
  useEffect(() => {
    if (!document.querySelector('script[src="/__review/bridge.js"]') || !state.project) return;
    try {
      sessionStorage.setItem('iris:review-page', page);
      sessionStorage.setItem('iris:review-project', String(state.project.id));
      if (state.focusedId !== null)
        sessionStorage.setItem('iris:review-photo', String(state.focusedId));
    } catch {
      /* Optional. */
    }
  }, [page, state.project?.id, state.focusedId]);
  useEffect(() => {
    if (!document.querySelector('script[src="/__review/bridge.js"]') || store.getSnapshot().project)
      return;
    let active = true;
    try {
      const id = Number(sessionStorage.getItem('iris:review-project'));
      const photo = Number(sessionStorage.getItem('iris:review-photo'));
      const saved = sessionStorage.getItem('iris:review-page');
      if (id > 0)
        void store
          .openRecentProject(id)
          .then(() => {
            if (!active) return;
            if (photo > 0) store.focus(photo);
            setPage(
              ['color', 'retouch', 'export'].includes(saved ?? '') ? (saved as AppPage) : 'photos'
            );
          })
          .catch(() => {});
    } catch {
      /* A review restart can use the normal project picker. */
    }
    return () => {
      active = false;
    };
  }, [store]);
  const [appearance, setAppearance] = useState<Appearance>(loadAppearance);
  const [error, setError] = useState<string | null>(null);
  const [scanRoot, setScanRoot] = useState<string | null>(null);
  const [inaccessible, setInaccessible] = useState<{ project: Project; reason: string } | null>(
    null
  );
  const [busy, setBusy] = useState(false);
  const observedJob = useRef<string | null>(null);
  useEffect(() => {
    const progress = state.progress;
    if (!progress || progress.kind === 'none') return;
    const key = `${state.project?.id}:${progress.id}`;
    if (progress.state === 'running' && observedJob.current !== key) {
      observedJob.current = key;
      recordDiagnostic('task_started');
    } else if (
      observedJob.current === key &&
      ['completed', 'failed', 'cancelled'].includes(progress.state)
    ) {
      observedJob.current = null;
      recordDiagnostic(
        progress.state === 'failed'
          ? 'task_failed'
          : progress.state === 'cancelled'
            ? 'task_cancelled'
            : 'task_completed'
      );
    }
  }, [state.project?.id, state.progress?.id, state.progress?.state]);
  useEffect(() => {
    if (state.connection === 'disconnected') recordDiagnostic('connection_lost');
  }, [state.connection]);
  const [tasksOpen, setTasksOpen] = useState(false);
  const taskPanel = useRef<HTMLDivElement>(null);
  const taskTrigger = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!tasksOpen) return;
    taskPanel.current?.querySelector<HTMLButtonElement>('button')?.focus();
    const outside = (event: PointerEvent) => {
      if (
        event.target instanceof Node &&
        !taskPanel.current?.contains(event.target) &&
        !taskTrigger.current?.contains(event.target)
      )
        setTasksOpen(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        setTasksOpen(false);
        taskTrigger.current?.focus();
      }
    };
    document.addEventListener('pointerdown', outside);
    document.addEventListener('keydown', escape);
    return () => {
      document.removeEventListener('pointerdown', outside);
      document.removeEventListener('keydown', escape);
    };
  }, [tasksOpen]);
  const [settingsDirty, setSettingsDirty] = useState(false);
  const [colorDirty, setColorDirty] = useState(false);
  const saveColor = useRef<(() => Promise<boolean>) | null>(null);
  const registerColorSave = useCallback((save: (() => Promise<boolean>) | null) => {
    saveColor.current = save;
  }, []);
  const dirty = settingsDirty || colorDirty;
  useEffect(() => {
    document.documentElement.dataset.irisEditingDirty = String(dirty);
  }, [dirty]);
  const desktopWindow = useDesktopWindow();
  const [leaveTarget, setLeaveTarget] = useState<AppPage | null>(null);
  const [savingLeave, setSavingLeave] = useState(false);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const saveSettings = useRef<(() => Promise<boolean>) | null>(null);
  const registerSave = useCallback((save: (() => Promise<boolean>) | null) => {
    saveSettings.current = save;
  }, []);
  const navigate = (target: AppPage) => {
    if ((settingsDirty && target !== 'settings') || (colorDirty && target !== 'color'))
      setLeaveTarget(target);
    else setPage(target);
  };
  const attempt = (operation: () => Promise<unknown>) => {
    setError(null);
    store.clearError();
    void operation().catch((reason) =>
      setError(reason instanceof Error ? reason.message : String(reason))
    );
  };
  useEffect(() => {
    saveAppearance(appearance);
  }, [appearance]);
  useEffect(() => {
    const help = (event: KeyboardEvent) => {
      if (event.key !== 'F1' || document.querySelector('dialog[open]')) return;
      event.preventDefault();
      if (dirty) setLeaveTarget('help');
      else setPage('help');
    };
    window.addEventListener('keydown', help);
    return () => window.removeEventListener('keydown', help);
  }, [dirty]);
  useEffect(() => {
    if (error || state.error) recordDiagnostic('frontend_failed');
  }, [error, state.error]);
  useEffect(() => {
    void window.__TAURI__?.core
      .invoke('set_unsaved_changes', { dirty })
      .catch((reason) => setError(`关闭保护暂不可用：${String(reason)}`));
  }, [dirty]);
  const openRecent = async (project: Project) => {
    try {
      await store.openRecentProject(project.id);
      setInaccessible(null);
      setPage('photos');
    } catch (reason) {
      setInaccessible({
        project,
        reason: reason instanceof Error ? reason.message : String(reason),
      });
      store.clearError();
    }
  };
  const pickFolder = async () => {
    const bridge = window.__TAURI__?.core;
    if (!bridge) throw new Error('请在桌面应用中选择文件夹');
    const root = await selectProjectFolder(bridge);
    if (root) setScanRoot(root);
  };
  const startScan = async () => {
    if (!scanRoot || busy) return;
    setBusy(true);
    try {
      await store.openProject(scanRoot);
      await store.run('scan');
      setScanRoot(null);
      setPage('photos');
    } finally {
      setBusy(false);
    }
  };

  const running =
    !!state.progress && ['running', 'paused', 'cancelling'].includes(state.progress.state);
  const modal =
    !!scanRoot || shortcutsOpen || tasksOpen || !!desktopWindow.request || !!leaveTarget;
  const photoWorkspace = usePhotoWorkspace({
    store,
    commands,
    page,
    modal,
    attempt,
    setError,
    setShortcutsOpen,
    setScanRoot,
  });
  return (
    <div className="app">
      <header className="titlebar" data-tauri-drag-region>
        <div className="titlebar-project" data-tauri-drag-region>
          <div className="titlebar-brand" data-tauri-drag-region>
            <span className="brand-mark" aria-hidden="true" />
            <span className="brand" data-tauri-drag-region>
              伊人
            </span>
          </div>
          {state.project && (
            <button
              type="button"
              className="crumb"
              onClick={() => navigate('projects')}
              title={state.project.root}
            >
              <Icon name="folder" />
              <span className="crumb-name">{state.project.name}</span>
            </button>
          )}
        </div>
        <WorkflowNav page={page} hasProject={!!state.project} onNavigate={navigate} />
        <div className="titlebar-tools">
          <span
            className={`connection connection-${state.connection}`}
            role="status"
            title={state.connection === 'disconnected' ? '服务连接已断开' : '服务已连接'}
          >
            <span className="connection-dot" aria-hidden="true" />
            <span className="visually-hidden">
              {state.connection === 'disconnected' ? '服务连接已断开' : '服务已连接'}
            </span>
          </span>
          <button
            type="button"
            className="title-button"
            title="帮助与关于 (F1)"
            aria-label="帮助与关于"
            onClick={() => navigate('help')}
          >
            <Icon name="info" />
          </button>
          <button
            ref={taskTrigger}
            type="button"
            className="title-button"
            title="任务与运行详情"
            aria-label="任务"
            aria-expanded={tasksOpen}
            aria-controls="task-center"
            onClick={() => setTasksOpen((value) => !value)}
          >
            {running ? (
              <span className="activity-ring" aria-hidden="true" />
            ) : (
              <Icon name="schedule" />
            )}
          </button>
          <button
            type="button"
            className="title-button"
            title="设置"
            aria-label="设置"
            aria-current={page === 'settings' ? 'page' : undefined}
            onClick={() => navigate('settings')}
          >
            <Icon name="settings" />
          </button>
        </div>
      </header>
      {(error || state.error) && (
        <div role="alert" className="error-bar">
          <Icon name="warning" />
          <div>
            <span>操作未完成</span>
            <ErrorDetails error={error || state.error || ''} />
          </div>
          <span className="spacer" />
          <Button
            className="quiet"
            onClick={() => {
              setError(null);
              store.clearError();
            }}
          >
            关闭提示
          </Button>
        </div>
      )}
      {state.connection === 'disconnected' && (
        <div role="status" className="error-bar notice">
          <span className="activity-dot" aria-hidden="true" />
          <span>服务连接已断开，正在等待恢复</span>
        </div>
      )}

      {page === 'projects' && inaccessible && (
        <main className="page">
          <div className="page-content state-page">
            <Icon name="warning" className="state-icon" />
            <h1>文件夹无法访问</h1>
            <div>
              <h2>{inaccessible.project.name}</h2>
              <p className="filename muted">{inaccessible.project.root}</p>
            </div>
            <p className="muted">请检查连接、权限或文件夹位置。</p>
            <div className="row">
              <Button primary onClick={() => attempt(() => openRecent(inaccessible.project))}>
                <Icon name="refresh" />
                重试
              </Button>
              <Button
                onClick={() => {
                  setInaccessible(null);
                  store.clearError();
                }}
              >
                返回项目列表
              </Button>
            </div>
            <details>
              <summary>详细原因</summary>
              <p>{inaccessible.reason}</p>
            </details>
          </div>
        </main>
      )}

      {page === 'projects' && !inaccessible && (
        <main className="page projects-page">
          <div className="page-content projects-content">
            <div className="projects-heading">
              <h1>项目</h1>
              <Button primary disabled={busy} onClick={() => attempt(pickFolder)}>
                <Icon name="folder" />
                打开照片文件夹
              </Button>
            </div>
            <div className="section-heading">
              <h2>最近项目</h2>
            </div>
            {state.projects.length === 0 ? (
              <p className="project-empty">暂无最近项目</p>
            ) : (
              <ul className="project-list">
                {state.projects.map((project) => (
                  <li
                    key={project.id}
                    className={`project-card ${state.project?.id === project.id ? 'current' : ''}`}
                  >
                    <button
                      className="project-open"
                      title={project.root}
                      aria-label={`${project.name}，${project.root}`}
                      aria-current={state.project?.id === project.id ? 'true' : undefined}
                      onClick={() =>
                        state.project?.id === project.id
                          ? navigate('photos')
                          : attempt(() => openRecent(project))
                      }
                    >
                      <span className="project-glyph" aria-hidden="true">
                        <Icon name="folder" />
                      </span>
                      <span className="project-text">
                        <strong>{project.name}</strong>
                      </span>
                      <Icon name="chevron-right" className="project-chevron" />
                    </button>
                    <button
                      type="button"
                      className="project-remove"
                      aria-label={`移出列表：${project.name}`}
                      title="移出列表"
                      onClick={() => attempt(() => store.hideRecentProject(project.id))}
                    >
                      <Icon name="close" />
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </main>
      )}

      {page === 'settings' && (
        <main className="page settings-page">
          <div className="page-content">
            <header className="page-heading">
              <h1>设置</h1>
              {state.project && (
                <Button className="ghost" onClick={() => navigate('photos')}>
                  <Icon name="back" />
                  返回选片
                </Button>
              )}
            </header>
            <div className="settings-layout">
              <nav className="settings-nav" aria-label="设置分类">
                <a href="#appearance">
                  <Icon name="tune" />
                  外观
                </a>
                <a href="#analysis">
                  <Icon name="auto-awesome" />
                  模型与分析
                </a>
                {state.project && (
                  <a href="#storage">
                    <Icon name="storage" />
                    存储与缓存
                  </a>
                )}
              </nav>
              <div className="settings-body">
                <section id="appearance" className="settings-section">
                  <h2>外观</h2>
                  <div className="setting-row">
                    <span>字号</span>
                    <div className="segmented" role="group" aria-label="字号">
                      {(
                        [
                          ['small', '小'],
                          ['medium', '中'],
                          ['large', '大'],
                        ] as const
                      ).map(([value, label]) => (
                        <Button
                          key={value}
                          aria-pressed={appearance.size === value}
                          onClick={() => setAppearance({ ...appearance, size: value })}
                        >
                          {label}
                        </Button>
                      ))}
                    </div>
                  </div>
                  <div className="setting-row">
                    <span>界面密度</span>
                    <div className="segmented" role="group" aria-label="界面密度">
                      {(
                        [
                          ['compact', '紧凑'],
                          ['standard', '标准'],
                          ['relaxed', '宽松'],
                        ] as const
                      ).map(([value, label]) => (
                        <Button
                          key={value}
                          aria-pressed={appearance.density === value}
                          onClick={() => setAppearance({ ...appearance, density: value })}
                        >
                          {label}
                        </Button>
                      ))}
                    </div>
                  </div>
                  <CloseBehaviorSetting controller={desktopWindow} />
                </section>
                <div id="analysis">
                  <AnalysisSettings
                    store={store}
                    onDirty={setSettingsDirty}
                    onSaveReady={registerSave}
                  />
                </div>
                <div id="storage">
                  <CacheSettings store={store} />
                </div>
              </div>
            </div>
          </div>
        </main>
      )}

      {page === 'photos' && state.project && <PhotoWorkspace model={photoWorkspace} />}

      {(page === 'color' || page === 'retouch') && state.project && (
        <EditingWorkspace
          model={photoWorkspace}
          stage={page}
          onDirty={setColorDirty}
          registerSave={registerColorSave}
        />
      )}
      {page === 'export' && state.project && (
        <main className="page">
          <div className="page-content export-workspace">
            <h1>导出</h1>
            <RenderedExportPanel client={store.client} photo={photoWorkspace.current} />
            <FileWorkflows store={store} />
          </div>
        </main>
      )}

      {page === 'help' && (
        <HelpCenter
          onBack={() => navigate(state.project ? 'photos' : 'projects')}
          onShortcuts={() => setShortcutsOpen(true)}
        />
      )}

      <div
        ref={taskPanel}
        id="task-center"
        className={`task-center${tasksOpen ? ' open' : ''}`}
        role={tasksOpen ? 'region' : undefined}
        aria-label={tasksOpen ? '任务详情' : undefined}
      >
        {tasksOpen && (
          <div className="task-center-heading">
            <h2>任务</h2>
            <button
              type="button"
              className="title-button"
              aria-label="关闭任务详情"
              title="关闭任务详情"
              onClick={() => {
                setTasksOpen(false);
                taskTrigger.current?.focus();
              }}
            >
              <Icon name="close" />
            </button>
          </div>
        )}
        <TaskBar
          store={store}
          expanded={tasksOpen}
          onSettings={() => {
            setTasksOpen(false);
            navigate('settings');
          }}
        />
        <ModelTaskBar
          store={store}
          expanded={tasksOpen}
          onSettings={() => {
            setTasksOpen(false);
            navigate('settings');
          }}
        />
      </div>
      {shortcutsOpen && (
        <Dialog title="键盘快捷键" onClose={() => setShortcutsOpen(false)}>
          <ShortcutSheet />
          <div className="dialog-actions">
            <Button primary onClick={() => setShortcutsOpen(false)}>
              知道了
            </Button>
          </div>
        </Dialog>
      )}
      {scanRoot && (
        <Dialog
          title="扫描照片文件夹"
          onClose={() => {
            if (!busy) setScanRoot(null);
          }}
        >
          <div className="stack">
            <p className="path-box filename">{scanRoot}</p>
            <dl>
              <dt>扫描范围</dt>
              <dd>当前文件夹及子文件夹</dd>
              <dt>文件处理</dt>
              <dd>保留原位置</dd>
            </dl>
          </div>
          {(error || state.error) && (
            <p role="alert" className="inline-error">
              {error || state.error}
            </p>
          )}
          <div className="dialog-actions">
            <Button disabled={busy} onClick={() => setScanRoot(null)}>
              取消
            </Button>
            <Button primary disabled={busy} onClick={() => attempt(startScan)}>
              {busy ? '正在打开' : '开始扫描'}
            </Button>
          </div>
        </Dialog>
      )}
      {leaveTarget && (
        <Dialog
          title="保存更改？"
          onClose={() => {
            if (!savingLeave) setLeaveTarget(null);
          }}
        >
          <p>{colorDirty ? '当前照片的调色尚未保存。' : '分析参数尚未保存。'}</p>
          <div className="dialog-actions">
            <Button disabled={savingLeave} onClick={() => setLeaveTarget(null)}>
              继续编辑
            </Button>
            <Button
              disabled={savingLeave}
              onClick={() => {
                setPage(leaveTarget);
                setSettingsDirty(false);
                setColorDirty(false);
                setLeaveTarget(null);
              }}
            >
              放弃更改
            </Button>
            <Button
              primary
              disabled={savingLeave}
              onClick={() => {
                setSavingLeave(true);
                setError(null);
                void (async () => {
                  try {
                    if (await (colorDirty ? saveColor.current?.() : saveSettings.current?.())) {
                      setPage(leaveTarget);
                      setSettingsDirty(false);
                      setColorDirty(false);
                      setLeaveTarget(null);
                    } else setError('保存未完成，请继续编辑并检查参数');
                  } catch (reason) {
                    setError(String(reason));
                  } finally {
                    setSavingLeave(false);
                  }
                })();
              }}
            >
              {savingLeave ? '正在保存' : '保存后返回'}
            </Button>
          </div>
          {error && (
            <p role="alert" className="inline-error">
              {error}
            </p>
          )}
        </Dialog>
      )}
      <WindowCloseDialog
        controller={desktopWindow}
        dirty={dirty}
        save={async () =>
          (await (colorDirty ? saveColor.current?.() : saveSettings.current?.())) ?? false
        }
      />
    </div>
  );
}
