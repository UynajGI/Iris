import { useState } from 'react';
import type { IrisClient } from '../../client.js';
import type { EditDocument } from '../../editing-client.js';
import { applyColorPreset, colorPresetRecipe } from '../../editing-presets.js';
import { Panel } from '../Panel.js';
import { Button } from '../components.js';
import { Icon } from '../Icon.js';
import { useColorPresets } from './useColorPresets.js';

export function PresetPanel({
  client,
  document,
  disabled,
  onChange,
}: {
  client: IrisClient;
  document: EditDocument | null;
  disabled: boolean;
  onChange(document: EditDocument): void;
}) {
  const presets = useColorPresets(client);
  const [selected, setSelected] = useState('');
  const [mode, setMode] = useState<'create' | 'rename' | 'delete' | null>(null);
  const [name, setName] = useState('');
  const [message, setMessage] = useState('');
  const current = presets.items.find((item) => item.id === selected);
  const blocked = disabled || presets.busy || !document;
  const save = async () => {
    if (!document || !name.trim() || blocked) return;
    const result = await presets.mutate(() =>
      mode === 'rename' && current
        ? presets.api.renamePreset(current, name.trim())
        : presets.api.savePreset(name.trim(), colorPresetRecipe(document))
    );
    if (result?.result) {
      setSelected(result.result.id);
      setMode(null);
      setMessage('预设已保存');
    }
  };
  const remove = async () => {
    if (!current || blocked) return;
    const result = await presets.mutate(async () => {
      await presets.api.deletePreset(current.id);
      return null;
    }, current.id);
    if (result) {
      setSelected('');
      setMode(null);
      setMessage('预设已删除');
    }
  };
  return (
    <Panel
      id="grade-presets"
      title="预设"
      defaultOpen={false}
      aside={
        <Button
          className="quiet icon-only"
          aria-label="刷新预设"
          title="刷新预设"
          disabled={presets.busy}
          onClick={() => {
            setMode(null);
            setSelected('');
            setMessage('');
            void presets.load();
          }}
        >
          <Icon name="refresh" />
        </Button>
      }
    >
      <div className="grade-presets">
        <label htmlFor="grade-preset-choice">全局影调、颜色与细节</label>
        <div className="grade-preset-choice">
          <select
            id="grade-preset-choice"
            aria-label="调色预设"
            value={selected}
            disabled={presets.busy}
            onChange={(event) => {
              setSelected(event.target.value);
              setMode(null);
              setMessage('');
            }}
          >
            <option value="">{presets.items.length ? '选择预设' : '暂无预设'}</option>
            {presets.items.map((item) => (
              <option key={item.id} value={item.id}>
                {item.name}
              </option>
            ))}
          </select>
          <Button
            disabled={blocked || !current}
            onClick={() => {
              if (!document || !current) return;
              try {
                onChange(applyColorPreset(document, current.recipe));
                setMessage('');
              } catch (reason) {
                setMessage(reason instanceof Error ? reason.message : String(reason));
              }
            }}
          >
            应用
          </Button>
        </div>
        <div className="grade-preset-actions">
          <Button
            className="quiet"
            disabled={blocked}
            onClick={() => {
              setMode('create');
              setName('');
              setMessage('');
            }}
          >
            新建预设
          </Button>
          <Button
            className="quiet"
            disabled={blocked || !current}
            onClick={() => {
              setMode('rename');
              setName(current!.name);
              setMessage('');
            }}
          >
            重命名
          </Button>
          <Button
            className="quiet"
            disabled={blocked || !current}
            onClick={() => {
              setMode('delete');
              setMessage('');
            }}
          >
            删除
          </Button>
        </div>
        {presets.more && (
          <Button className="quiet" disabled={presets.busy} onClick={() => void presets.load(true)}>
            加载更多预设
          </Button>
        )}
        {(mode === 'create' || mode === 'rename') && (
          <form
            className="grade-preset-form"
            onSubmit={(event) => {
              event.preventDefault();
              void save();
            }}
          >
            <label htmlFor="grade-preset-name">
              {mode === 'create' ? '保存当前调色' : '重命名预设'}
            </label>
            <input
              id="grade-preset-name"
              aria-label="预设名称"
              autoFocus
              maxLength={80}
              value={name}
              onChange={(event) => setName(event.target.value)}
              disabled={presets.busy}
            />
            <div className="grade-preset-actions">
              <Button type="submit" disabled={blocked || !name.trim()}>
                保存预设
              </Button>
              <Button className="quiet" disabled={presets.busy} onClick={() => setMode(null)}>
                取消
              </Button>
            </div>
          </form>
        )}
        {mode === 'delete' && current && (
          <div className="grade-preset-form">
            <span>删除预设“{current.name}”？已应用的照片不受影响。</span>
            <div className="grade-preset-actions">
              <Button disabled={blocked} onClick={() => void remove()}>
                确认删除预设
              </Button>
              <Button className="quiet" disabled={presets.busy} onClick={() => setMode(null)}>
                取消
              </Button>
            </div>
          </div>
        )}
        {presets.error && (
          <p className="grade-error" role="alert">
            {presets.error}
          </p>
        )}
        <span role="status">{presets.busy ? '正在读取或保存预设' : message}</span>
      </div>
    </Panel>
  );
}
