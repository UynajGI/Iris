import { useEffect, useState } from 'react';
import { Button, Dialog } from './components.js';

type CloseBehavior = 'ask' | 'background' | 'exit';
type CloseRequest = { forceExit: boolean };

export function useDesktopWindow() {
  const [request, setRequest] = useState<CloseRequest | null>(null);
  const [behavior, setBehavior] = useState<CloseBehavior>('ask');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    const bridge = window.__TAURI__;
    if (!bridge) return;
    let active = true;
    const listeners = [
      bridge.event.listen<CloseRequest>('iris:close-requested', ({ payload }) => {
        setError(null);
        setRequest((current) => current ?? payload);
      }),
      bridge.event.listen<string>('iris:window-error', ({ payload }) => setError(payload)),
    ];
    void bridge.core
      .invoke<CloseBehavior>('close_behavior')
      .then((value) => {
        if (active) setBehavior(value);
      })
      .catch((reason) => setError(String(reason)));
    void bridge.core
      .invoke<string>('window_frame_platform')
      .then((value) => {
        if (active) document.documentElement.dataset.windowFrame = value;
      })
      .catch((reason) => setError(String(reason)));
    for (const listener of listeners) void listener.catch((reason) => setError(String(reason)));
    return () => {
      active = false;
      for (const listener of listeners) void listener.then((stop) => stop()).catch(() => {});
    };
  }, []);
  const changeBehavior = async (value: CloseBehavior) => {
    setBusy(true);
    setError(null);
    try {
      await window.__TAURI__?.core.invoke('set_close_behavior', { behavior: value });
      setBehavior(value);
    } catch (reason) {
      setError(`无法保存关闭行为：${String(reason)}`);
    } finally {
      setBusy(false);
    }
  };
  const confirm = async (action: 'background' | 'exit', remember: boolean) => {
    setBusy(true);
    setError(null);
    try {
      await window.__TAURI__?.core.invoke('confirm_close', { action, remember });
      if (remember) setBehavior(action);
      setRequest(null);
    } catch (reason) {
      setError(`无法关闭窗口：${String(reason)}`);
    } finally {
      setBusy(false);
    }
  };
  return {
    request,
    behavior,
    changeBehavior,
    confirm,
    busy,
    error,
    cancel: () => {
      if (!busy) {
        setRequest(null);
        setError(null);
      }
    },
  };
}

type WindowController = ReturnType<typeof useDesktopWindow>;

export function CloseBehaviorSetting({ controller }: { controller: WindowController }) {
  if (!window.__TAURI__) return null;
  return (
    <div className="setting-row">
      <label htmlFor="close-behavior">关闭主窗口</label>
      <select
        id="close-behavior"
        value={controller.behavior}
        disabled={controller.busy}
        onChange={(event) => void controller.changeBehavior(event.target.value as CloseBehavior)}
      >
        <option value="ask">每次询问</option>
        <option value="background">后台运行</option>
        <option value="exit">退出程序</option>
      </select>
      {controller.error && (
        <p role="alert" className="inline-error">
          {controller.error}
        </p>
      )}
    </div>
  );
}

export function WindowCloseDialog({
  controller,
  dirty,
  save,
}: {
  controller: WindowController;
  dirty: boolean;
  save(): Promise<boolean>;
}) {
  const [remember, setRemember] = useState(false);
  const [exitSelected, setExitSelected] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    setRemember(false);
    setExitSelected(false);
    setSaveError(null);
  }, [controller.request]);
  if (!controller.request) return null;
  const exiting = controller.request.forceExit || exitSelected;
  const guarded = exiting && dirty;
  const busy = controller.busy || saving;
  const exit = () => {
    if (dirty) setExitSelected(true);
    else void controller.confirm('exit', remember);
  };
  const saveAndExit = async () => {
    setSaving(true);
    setSaveError(null);
    try {
      if (await save()) await controller.confirm('exit', remember);
      else setSaveError('保存未完成，请检查分析参数。');
    } catch (reason) {
      setSaveError(String(reason));
    } finally {
      setSaving(false);
    }
  };
  return (
    <Dialog
      title={guarded ? '保存更改后退出？' : '关闭伊人？'}
      onClose={() => {
        if (!busy) controller.cancel();
      }}
    >
      <p>{guarded ? '分析参数尚未保存。' : '后台运行可从系统托盘重新打开，任务会继续运行。'}</p>
      {!exiting && (
        <label className="close-remember">
          <input
            type="checkbox"
            checked={remember}
            disabled={busy}
            onChange={(event) => setRemember(event.target.checked)}
          />
          不再提示，记住本次选择
        </label>
      )}
      <div className="dialog-actions">
        <Button disabled={busy} onClick={controller.cancel}>
          {guarded ? '继续编辑' : '取消'}
        </Button>
        {guarded ? (
          <>
            <Button disabled={busy} onClick={() => void controller.confirm('exit', remember)}>
              放弃更改并退出
            </Button>
            <Button primary disabled={busy} onClick={() => void saveAndExit()}>
              {saving ? '正在保存' : '保存并退出'}
            </Button>
          </>
        ) : (
          <>
            <Button disabled={busy} onClick={exit}>
              退出程序
            </Button>
            <Button
              primary
              disabled={busy}
              onClick={() => void controller.confirm('background', remember)}
            >
              后台运行
            </Button>
          </>
        )}
      </div>
      {(controller.error || saveError) && (
        <p role="alert" className="inline-error">
          {controller.error || saveError}
        </p>
      )}
    </Dialog>
  );
}
