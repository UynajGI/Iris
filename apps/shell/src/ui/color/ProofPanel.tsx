import { useEffect, useRef, useState } from 'react';
import type { IrisClient } from '../../client.js';
import { EditingClient } from '../../editing-client.js';
import { ApiError } from '../../api-error.js';
import { Panel } from '../Panel.js';
import { Button } from '../components.js';

export type ProofProfile = { hash: string; name: string; enabled: boolean };
type Props = {
  client: IrisClient;
  profile: ProofProfile | null;
  disabled: boolean;
  onChange(profile: ProofProfile | null): void;
};

function importError(reason: unknown): string {
  if (!(reason instanceof ApiError))
    return reason instanceof Error ? reason.message : String(reason);
  if (reason.message.includes('quota')) return '调色资源存储空间已达上限';
  if (reason.status === 400) return 'ICC 配置无法导入，请选择有效的 RGB、灰度或 CMYK 配置文件';
  return 'ICC 导入失败，请检查后台连接后重试';
}

function useProofImport({ client, onChange }: Props) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const generation = useRef(0);
  const locked = useRef(false);
  useEffect(() => {
    const current = ++generation.current;
    locked.current = false;
    setBusy(false);
    setError('');
    return () => {
      generation.current = current + 1;
    };
  }, [client]);
  const upload = async (file: File) => {
    if (locked.current) return;
    const current = generation.current;
    locked.current = true;
    setBusy(true);
    setError('');
    try {
      if (!/\.(icc|icm)$/i.test(file.name)) throw new Error('请选择 .icc 或 .icm 配置文件');
      if (file.size === 0 || file.size > 16 * 1024 * 1024)
        throw new Error('ICC 配置文件须为 1 字节至 16 MB');
      const resource = await new EditingClient(client).importIcc(file);
      if (current === generation.current)
        onChange({ hash: resource.hash, name: file.name, enabled: true });
    } catch (reason) {
      if (current === generation.current) setError(importError(reason));
    } finally {
      if (current === generation.current) {
        locked.current = false;
        setBusy(false);
      }
    }
  };
  return { busy, error, upload };
}

export function ProofPanel(props: Props) {
  const { profile, disabled, onChange } = props;
  const { busy, error, upload } = useProofImport(props);
  return (
    <Panel id="grade-proof" title="软打样">
      <fieldset className="grade-adjustments" disabled={disabled || busy}>
        <legend className="visually-hidden">ICC 软打样</legend>
        <label className="grade-output-check">
          <input
            type="checkbox"
            disabled={!profile}
            checked={!!profile?.enabled}
            onChange={(event) => {
              if (profile) onChange({ ...profile, enabled: event.target.checked });
            }}
          />
          启用软打样
        </label>
        {profile && (
          <p className="filename-line" title={profile.name}>
            {profile.name}
          </p>
        )}
        <div className="grade-lens-actions">
          <label className="grade-resource-import">
            {busy ? '正在导入' : profile ? '更换 ICC' : '导入 ICC'}
            <input
              type="file"
              accept=".icc,.icm"
              aria-label="导入打样 ICC"
              onChange={(event) => {
                const file = event.target.files?.[0];
                event.target.value = '';
                if (file) void upload(file);
              }}
            />
          </label>
          {profile && (
            <Button className="quiet" onClick={() => onChange(null)}>
              移除
            </Button>
          )}
        </div>
        <p className="muted">仅影响预览，不改变导出设置。</p>
      </fieldset>
      {error && <p role="alert">{error}</p>}
    </Panel>
  );
}
