import { useEffect, useRef, useState } from 'react';
import type { SceneGroup } from '../types.js';
import type { PhotoWorkspaceModel } from './usePhotoWorkspace.js';
import { Button } from './components.js';

function useSceneBrowser(model: PhotoWorkspaceModel) {
  const { store, state } = model;
  const [scenes, setScenes] = useState<SceneGroup[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [page, setPage] = useState(0);
  const request = useRef<AbortController | null>(null);
  useEffect(() => () => request.current?.abort(), []);
  const read = async () => {
    request.current?.abort();
    const controller = new AbortController();
    request.current = controller;
    setBusy(true);
    setError('');
    try {
      const result = await store.client.scenes(state.project!.id, controller.signal);
      if (!controller.signal.aborted) {
        setScenes(result);
        setPage(0);
      }
    } catch (cause) {
      if (!controller.signal.aborted) setError(String(cause));
    } finally {
      if (!controller.signal.aborted) setBusy(false);
    }
  };
  const selected = state.selectedIds.length
    ? state.selectedIds
    : (model.group?.memberIds ?? (model.current ? [model.current.id] : []));
  const correct = async (ids: number[], reset = false) => {
    request.current?.abort();
    const controller = new AbortController();
    request.current = controller;
    setBusy(true);
    setError('');
    try {
      const result = await store.client.correctScene(state.project!.id, { photo_ids: ids, reset });
      if (controller.signal.aborted) return;
      setScenes(result);
      setPage(0);
      // Corrections change stable scene ids; leave the previous scope explicitly.
      if (state.filter.scene_id) {
        const { scene_id: _, ...filter } = state.filter;
        await store.setFilter(filter);
      }
    } catch (cause) {
      if (!controller.signal.aborted) setError(String(cause));
    } finally {
      if (!controller.signal.aborted) setBusy(false);
    }
  };
  return { scenes, busy, error, page, setPage, read, selected, correct };
}

export function SceneBrowser({ model }: { model: PhotoWorkspaceModel }) {
  const { store, state, attempt } = model;
  const { scenes, busy, error, page, setPage, read, selected, correct } = useSceneBrowser(model);
  return (
    <div className="scene-browser">
      <p className="muted nav-note">场景候选依据时间与画面结构；连拍和重复文件各自保留。</p>
      <Button className="ghost block" disabled={busy || model.running} onClick={() => void read()}>
        {busy ? '正在读取…' : scenes ? '刷新场景' : '查看场景分组'}
      </Button>
      {state.filter.scene_id && (
        <Button
          className="quiet block"
          onClick={() => {
            const { scene_id: _, ...filter } = state.filter;
            attempt(() => store.setFilter(filter));
          }}
        >
          退出场景范围
        </Button>
      )}
      {error && (
        <p className="inline-error" role="alert">
          场景操作未完成，可重试：{error}
        </p>
      )}
      {scenes && (
        <>
          <p className="nav-note muted tabular">{scenes.length} 个分组 · 分析后可刷新候选</p>
          <SceneList model={model} scenes={scenes} page={page} />
          {scenes.length > 30 && (
            <div className="compact-actions">
              <Button disabled={!page} onClick={() => setPage(page - 1)}>
                上一页
              </Button>
              <span className="tabular">
                {page + 1}/{Math.ceil(scenes.length / 30)}
              </span>
              <Button disabled={(page + 1) * 30 >= scenes.length} onClick={() => setPage(page + 1)}>
                下一页
              </Button>
            </div>
          )}
          <div className="scene-corrections">
            <p className="muted nav-note">选中照片 {selected.length} 张 · 手动修正保留到下次打开</p>
            <Button
              className="ghost block"
              disabled={busy || model.running || selected.length < 2}
              onClick={() => void correct(selected)}
            >
              将所选合为一个场景
            </Button>
            <Button
              className="quiet block"
              disabled={busy || model.running || !model.current}
              onClick={() => void correct([model.current!.id])}
            >
              将当前照片拆出
            </Button>
            <Button
              className="quiet block"
              disabled={busy || model.running || !selected.length}
              onClick={() => void correct(selected, true)}
            >
              恢复所选自动分组
            </Button>
          </div>
        </>
      )}
    </div>
  );
}

function SceneList({
  model,
  scenes,
  page,
}: {
  model: PhotoWorkspaceModel;
  scenes: SceneGroup[];
  page: number;
}) {
  const { store, state, attempt, setView } = model;
  return (
    <div className="scene-list nav-list">
      {scenes.slice(page * 30, (page + 1) * 30).map((scene, index) => (
        <div key={scene.id}>
          <button
            type="button"
            className="nav-item"
            aria-pressed={state.filter.scene_id === scene.id}
            onClick={() => {
              setView('grid');
              const { photo_id: _, ...filter } = state.filter;
              attempt(() => store.setFilter({ ...filter, scene_id: scene.id, offset: 0 }));
            }}
          >
            <span>
              {scene.kind === 'manual'
                ? '手动场景'
                : scene.kind === 'unsequenced'
                  ? '时间未知'
                  : '场景候选'}{' '}
              {page * 30 + index + 1}
            </span>
            <span className="nav-count tabular">{scene.member_photo_ids.length}</span>
          </button>
          {scene.burst_group_ids.map((id, i) => (
            <button
              type="button"
              key={id}
              className="nav-item scene-burst"
              disabled={model.running || state.reanalysisRequired}
              onClick={() => attempt(() => store.openGroup(id))}
            >
              连拍 {i + 1}
            </button>
          ))}
        </div>
      ))}
    </div>
  );
}
