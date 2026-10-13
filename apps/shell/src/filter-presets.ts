import type { PhotoFilter } from './types.js';
import { validatedPhotoFilter, type PreferenceStorage } from './view-preferences.js';

export interface FilterPreset {
  name: string;
  filter: PhotoFilter;
}
export const FILTER_PRESET_KEY = 'iris:filter-presets:v1';
export const MAX_FILTER_PRESETS = 20;

/** Presets travel between projects; never retain photo IDs, scene scopes or pagination. */
export function presetFilter(value: unknown): PhotoFilter {
  const { limit: _limit, include_missing: _missing, ...filter } = validatedPhotoFilter(value);
  return filter;
}
export function applyFilterPreset(current: PhotoFilter, saved: PhotoFilter): PhotoFilter {
  return { ...presetFilter(saved), limit: current.limit ?? 200, offset: 0 };
}
function storage(): PreferenceStorage | undefined {
  try {
    return window.localStorage;
  } catch {
    return undefined;
  }
}
function validName(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0 && value.length <= 40;
}
export class FilterPresetsRepository {
  constructor(private readonly backing: PreferenceStorage | undefined = storage()) {}

  load(): FilterPreset[] {
    try {
      const text = this.backing?.getItem(FILTER_PRESET_KEY);
      if (!text || text.length > 16384) return [];
      const data = JSON.parse(text);
      if (data.version !== 1 || !Array.isArray(data.presets)) return [];
      const names = new Set<string>();
      const result: FilterPreset[] = [];
      for (const item of data.presets.slice(0, MAX_FILTER_PRESETS)) {
        if (!item || !validName(item.name) || names.has(item.name.trim())) continue;
        const name = item.name.trim();
        names.add(name);
        result.push({ name, filter: presetFilter(item.filter) });
      }
      return result;
    } catch {
      return [];
    }
  }
  add(name: string, filter: PhotoFilter): FilterPreset[] {
    name = name.trim();
    if (!validName(name)) throw new Error('请输入 1–40 字的组合名称');
    const presets = this.load();
    if (presets.some((preset) => preset.name === name))
      throw new Error('名称已存在，请换一个名称，或先删除原组合');
    if (presets.length >= MAX_FILTER_PRESETS)
      throw new Error('最多保存 20 个组合，请先删除不用的组合');
    return this.save([...presets, { name, filter: presetFilter(filter) }]);
  }
  remove(name: string): FilterPreset[] {
    return this.save(this.load().filter((preset) => preset.name !== name));
  }
  private save(presets: FilterPreset[]): FilterPreset[] {
    try {
      if (!this.backing) throw new Error('Storage unavailable');
      this.backing.setItem(FILTER_PRESET_KEY, JSON.stringify({ version: 1, presets }));
    } catch {
      throw new Error('无法保存常用组合，请检查应用存储空间后重试');
    }
    return presets;
  }
}
