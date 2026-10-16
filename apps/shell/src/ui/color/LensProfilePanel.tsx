import { useEffect, useMemo, useRef, useState } from 'react';
import type { IrisClient } from '../../client.js';
import { EditingClient, type EditDocument, type EditRecord } from '../../editing-client.js';
import {
  applyLensProfile,
  installedLensProfiles,
  lensProfileNode,
  rememberLensProfiles,
  type InstalledLensProfile,
} from '../../editing-lens.js';
import { ApiError } from '../../api-error.js';
import { Button } from '../components.js';

export function LensProfilePanel({
  client,
  record,
  document,
  disabled,
  onChange,
}: {
  client: IrisClient;
  record: EditRecord | null;
  document: EditDocument | null;
  disabled: boolean;
  onChange(document: EditDocument): void;
}) {
  const api = useMemo(() => new EditingClient(client), [client]);
  const [profiles, setProfiles] = useState(installedLensProfiles);
  const [chosen, setChosen] = useState('');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const latest = useRef({ record, document });
  latest.current = { record, document };
  const generation = useRef(0);
  useEffect(() => {
    ++generation.current;
    setMessage('');
    setBusy(false);
    return () => {
      ++generation.current;
    };
  }, [record?.id]);
  const node = lensProfileNode(document);
  const match = async (candidates: InstalledLensProfile[], current: number) => {
    const base = latest.current.record;
    if (!base) return;
    const matches = [];
    for (const profile of candidates) {
      try {
        const result = await api.lensProfile(base, profile.hash);
        if (generation.current !== current) return;
        if (result.matching.applicable) matches.push(result);
      } catch (reason) {
        if (!(reason instanceof ApiError && reason.status === 404)) throw reason;
      }
    }
    if (generation.current !== current) return;
    if (!matches.length) throw new Error('没有匹配的配置，请核对相机、镜头、焦段、光圈和原片尺寸');
    if (matches.length > 1) throw new Error('匹配到多个配置，请先选择一个配置');
    const state = latest.current;
    if (!state.record || !state.document) return;
    onChange(applyLensProfile(state.record, state.document, matches[0]!));
    setMessage('已匹配并应用镜头配置');
  };
  const run = async (action: (current: number) => Promise<void>) => {
    if (busy || disabled) return;
    const current = generation.current;
    setBusy(true);
    setMessage('');
    try {
      await action(current);
    } catch (reason) {
      if (generation.current === current)
        setMessage(reason instanceof Error ? reason.message : String(reason));
    } finally {
      if (generation.current === current) setBusy(false);
    }
  };
  return (
    <div className="grade-lens-profile">
      <label className="grade-output-check">
        <input
          type="checkbox"
          checked={!!node?.enabled}
          disabled={disabled || busy || !node}
          onChange={(event) => {
            if (document && node)
              onChange({
                ...document,
                nodes: document.nodes.map((item) =>
                  item.id === node.id ? { ...item, enabled: event.target.checked } : item
                ),
              });
          }}
        />
        镜头配置校正
      </label>
      <select
        aria-label="镜头配置"
        value={chosen}
        disabled={disabled || busy}
        onChange={(event) => setChosen(event.target.value)}
      >
        <option value="">自动匹配已导入配置</option>
        {profiles.map((item) => (
          <option value={item.hash} key={item.hash}>
            {item.name}
          </option>
        ))}
      </select>
      <div className="grade-lens-actions">
        <Button
          disabled={disabled || busy || !profiles.length}
          onClick={() =>
            void run((current) =>
              match(chosen ? profiles.filter((item) => item.hash === chosen) : profiles, current)
            )
          }
        >
          {busy ? '正在匹配' : '匹配并应用'}
        </Button>
        <label className="grade-resource-import">
          导入配置
          <input
            type="file"
            accept=".json,application/json"
            aria-label="导入镜头配置"
            disabled={disabled || busy}
            onChange={(event) => {
              const file = event.target.files?.[0];
              event.target.value = '';
              if (!file) return;
              void run(async (current) => {
                if (file.size > 32 * 1024 * 1024) throw new Error('镜头配置文件超过 32 MB');
                const imported = await api.importLensProfile(file);
                if (generation.current !== current) return;
                const item = { hash: imported.hash, name: file.name.slice(0, 160) };
                const next = [
                  item,
                  ...profiles.filter((profile) => profile.hash !== item.hash),
                ].slice(0, 32);
                setProfiles(next);
                rememberLensProfiles(next);
                setChosen(item.hash);
                await match([item], current);
              });
            }}
          />
        </label>
      </div>
      {!profiles.length && <p className="muted">尚未导入镜头配置</p>}
      {message && <p role="status">{message}</p>}
    </div>
  );
}
