import { Select } from './Select.js';
import { useState } from 'react';
import { applyFilterPreset, FilterPresetsRepository, presetFilter } from '../filter-presets.js';
import type { PhotoFilter } from '../types.js';
import { Button } from './components.js';
import './filter-presets.css';

export function FilterPresets({
  filter,
  onChange,
}: {
  filter: PhotoFilter;
  onChange(filter: PhotoFilter): void;
}) {
  const [repository] = useState(() => new FilterPresetsRepository());
  const [presets, setPresets] = useState(() => repository.load());
  const [selected, setSelected] = useState('');
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState('');
  const [notice, setNotice] = useState('');
  const [error, setError] = useState('');
  const active =
    presets.find(
      (preset) =>
        preset.name === selected &&
        JSON.stringify(presetFilter(preset.filter)) === JSON.stringify(presetFilter(filter))
    )?.name ?? '';
  const mutate = (operation: () => void) => {
    setNotice('');
    setError('');
    try {
      operation();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  };
  const save = () =>
    mutate(() => {
      setPresets(repository.add(name, filter));
      setSelected(name.trim());
      setName('');
      setEditing(false);
      setNotice('已保存组合');
    });
  return (
    <div className="filter-presets">
      {presets.length > 0 && (
        <>
          <label className="field">
            常用组合
            <Select
              aria-label="常用组合"
              value={active}
              onValueChange={(nextValue) => {
                const value = nextValue;
                setSelected(value);
                setNotice('');
                setError('');
                const preset = presets.find((item) => item.name === value);
                if (preset) onChange(applyFilterPreset(filter, preset.filter));
              }}
            >
              <option value="">选择组合</option>
              {presets.map((preset) => (
                <option key={preset.name} value={preset.name}>
                  {preset.name}
                </option>
              ))}
            </Select>
          </label>
        </>
      )}
      {editing ? (
        <div className="filter-preset-editor">
          <label htmlFor="filter-preset-name">组合名称</label>
          <input
            id="filter-preset-name"
            autoFocus
            value={name}
            maxLength={40}
            placeholder="例如：保留五星"
            onChange={(event) => setName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && !event.nativeEvent.isComposing) {
                event.preventDefault();
                save();
              }
            }}
          />
          <div className="filter-preset-actions">
            <Button
              className="quiet"
              onClick={() => {
                setEditing(false);
                setError('');
              }}
            >
              取消
            </Button>
            <Button disabled={!name.trim()} onClick={save}>
              保存组合
            </Button>
          </div>
        </div>
      ) : (
        <div className="filter-preset-actions">
          <Button
            className="quiet"
            onClick={() => {
              setEditing(true);
              setNotice('');
              setError('');
            }}
          >
            保存当前筛选
          </Button>
          {presets.length > 0 && (
            <Button
              className="quiet"
              disabled={!active}
              onClick={() =>
                mutate(() => {
                  setPresets(repository.remove(active));
                  setSelected('');
                  setNotice('已删除组合');
                })
              }
            >
              删除组合
            </Button>
          )}
        </div>
      )}
      {notice && <p role="status">{notice}</p>}
      {error && <p role="alert">{error}</p>}
    </div>
  );
}
