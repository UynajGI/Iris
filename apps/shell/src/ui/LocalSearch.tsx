import { useEffect, useRef, useState } from 'react';
import type { Photo, SearchResponse, SearchStatus } from '../types.js';
import { loadGroupMembers } from '../group-session.js';
import type { PhotoWorkspaceModel } from './usePhotoWorkspace.js';
import { Button, PhotoImage } from './components.js';

export function LocalSearch({ model }: { model: PhotoWorkspaceModel }) {
  const { store, state, attempt } = model;
  const [status, setStatus] = useState<SearchStatus | null>(null);
  const [query, setQuery] = useState('');
  const [result, setResult] = useState<SearchResponse | null>(null);
  const [photos, setPhotos] = useState<Photo[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const request = useRef<AbortController | null>(null);
  useEffect(() => () => request.current?.abort(), []);
  const run = async (operation: (controller: AbortController) => Promise<void>) => {
    request.current?.abort();
    const controller = new AbortController();
    request.current = controller;
    setBusy(true);
    setError('');
    try {
      await operation(controller);
    } catch (cause) {
      if (!controller.signal.aborted) setError(String(cause));
    } finally {
      if (!controller.signal.aborted) setBusy(false);
    }
  };
  const check = () =>
    run(async (controller) => {
      const next = await store.client.searchStatus(state.project!.id, controller.signal);
      if (!controller.signal.aborted) setStatus(next);
    });
  const search = () =>
    run(async (controller) => {
      setResult(null);
      setPhotos([]);
      const next = await store.client.search(
        state.project!.id,
        { query: query.trim(), limit: 24 },
        controller.signal
      );
      if (controller.signal.aborted) return;
      setResult(next);
      const members = await loadGroupMembers(
        store.client,
        state.project!.id,
        next.hits.map((hit) => hit.photo_id),
        () => !controller.signal.aborted
      );
      if (!controller.signal.aborted) setPhotos(members);
    });
  const index = () =>
    run(async (controller) => {
      await store.client.indexSearch(state.project!.id);
      if (!controller.signal.aborted) await store.refresh();
    });
  const ready = status?.model.state === 'ready';
  return (
    <div className="local-search">
      <p className="muted nav-note">本地以文搜图 · 可选功能，首次使用需准备兼容模型。</p>
      <Button className="ghost block" disabled={busy} onClick={() => void check()}>
        {busy ? '正在处理…' : status ? '刷新检索状态' : '检查本地检索模型'}
      </Button>
      {status && !ready && (
        <p className="nav-note" role="status">
          {status.model.state === 'disabled'
            ? '尚未启用本地检索模型。'
            : '本地模型未通过检查，请检查配置。'}{' '}
          选片和分析可继续使用；配置说明见随软件提供的本地搜索指南。
        </p>
      )}
      {ready && (
        <>
          <p className="nav-note muted tabular">
            {status.model.label} · 已索引 {status.coverage?.indexed ?? 0}/
            {status.coverage?.total ?? 0} 张
          </p>
          <Button
            className="quiet block"
            disabled={busy || model.running || !status.coverage?.pending}
            onClick={() => void index()}
          >
            建立 / 补全检索索引
          </Button>
          <form
            className="search-form"
            onSubmit={(event) => {
              event.preventDefault();
              void search();
            }}
          >
            <label className="field">
              描述要找的照片
              <input
                value={query}
                maxLength={1000}
                placeholder="例如：逆光下的人像"
                onChange={(event) => setQuery(event.target.value)}
              />
            </label>
            <Button primary disabled={busy || !query.trim() || !(status.coverage?.indexed ?? 0)}>
              搜索
            </Button>
          </form>
          <p className="muted nav-note">语言支持取决于所选模型；匹配度不是画质评分。</p>
        </>
      )}
      {error && (
        <p className="inline-error" role="alert">
          本地搜索未完成，可重试：{error}
        </p>
      )}
      {state.filter.photo_id && (
        <Button
          className="quiet block"
          onClick={() => attempt(() => store.setFilter({ limit: 200 }))}
        >
          返回全部照片
        </Button>
      )}
      {result && (
        <>
          <p className="nav-note muted tabular" role="status">
            {result.hits.length
              ? `${result.hits.length} 张结果 · 当前索引覆盖 ${result.coverage.indexed}/${result.coverage.total}`
              : '当前索引中没有结果'}
          </p>
          <div className="search-results">
            {result.hits.map((hit) => {
              const photo = photos.find((item) => item.id === hit.photo_id);
              return photo ? (
                <button
                  type="button"
                  key={photo.id}
                  aria-label={`查看搜索结果 ${photo.filename}`}
                  onClick={() => attempt(() => store.revealPhoto(photo.id))}
                >
                  <PhotoImage client={store.client} photo={photo} />
                  <span className="filename-line">{photo.filename}</span>
                  <span className="muted tabular">匹配度 {hit.similarity.toFixed(3)}</span>
                </button>
              ) : null;
            })}
          </div>
        </>
      )}
    </div>
  );
}
