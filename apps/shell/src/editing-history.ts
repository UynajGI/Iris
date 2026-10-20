import type { EditHistoryEntry } from './editing-client.js';

const labels: Record<string, string> = {
  exposure: '曝光',
  contrast: '对比度',
  highlights: '高光',
  shadows: '阴影',
  whites: '白色',
  blacks: '黑色',
  temperature: '冷暖',
  tint: '色调',
  saturation: '饱和度',
  vibrance: '自然饱和度',
  black_and_white: '黑白成片',
  master_curve: 'RGB 曲线',
  red_curve: '红色曲线',
  green_curve: '绿色曲线',
  blue_curve: '蓝色曲线',
  hsl: '颜色混合器',
  hue: '色相',
  luminance: '亮度',
  texture: '纹理',
  clarity: '清晰度',
  dehaze: '去雾',
  sharpening: '锐化',
  sharpening_threshold: '锐化阈值',
  luminance_denoise: '明亮度降噪',
  chroma_denoise: '颜色降噪',
  denoise_sigma: '降噪范围',
  crop: '裁切',
  rotation_degrees: '旋转',
  perspective: '透视',
  radial: '镜头畸变',
  chromatic_aberration: '色差校正',
  vignette_stops: '光学暗角',
  opacity: '不透明度',
  enabled: '效果开关',
  added: '添加',
  removed: '移除',
  raw_white_balance: 'RAW 白平衡',
  grain: '颗粒',
  vignette: '暗角',
  amount: '强度',
  midpoint: '中心范围',
  feather: '羽化',
  seed: '纹理变化',
  strength: '强度',
  color_balance: '颜色分级',
  global: '全局',
  midtones: '中间调',
  red: '红',
  green: '绿',
  blue: '蓝',
  lift: '暗部',
  gamma: '中间调',
  gain: '亮部',
  tone_curve: '参数曲线',
  darks: '暗调',
  lights: '亮调',
  tone: '仅影调',
  color: '仅颜色',
  separated: '',
  parameters: '',
  observation: '观察视图',
  start: '起点',
  end: '终点',
  center: '中心',
  radius: '范围',
  strokes: '画笔',
  hardness: '硬度',
  name: '重命名',
  inputs: '节点连接',
  output_node: '输出节点',
};
const actions: Record<string, string> = {
  create: '原始调整',
  save: '调整设置',
  undo: '撤销',
  redo: '重做',
  duplicate: '创建副本',
  import_recipe: '应用调整',
  batch: '批量同步',
};
export function historySummary(entry: EditHistoryEntry): string {
  const changes = Object.entries(entry.changes ?? {}).map(([path, value]) => {
    const parts = path.split('/');
    const named = parts.slice(1).filter((part) => part !== 'type' && part !== 'parameters');
    const label =
      named
        .map((part) => labels[part] ?? '')
        .filter(Boolean)
        .join(' · ') ||
      labels[path] ||
      parts[0] ||
      '调整设置';
    const numeric = value !== '' && Number.isFinite(Number(value));
    const key = parts.at(-1)!;
    const scale = ['contrast', 'highlights', 'shadows', 'whites', 'blacks'].includes(key)
      ? 50
      : [
            'temperature',
            'tint',
            'saturation',
            'vibrance',
            'texture',
            'clarity',
            'dehaze',
            'sharpening',
            'luminance_denoise',
            'chroma_denoise',
            'opacity',
          ].includes(key)
        ? 100
        : 1;
    const display =
      value === 'true'
        ? '开'
        : value === 'false'
          ? '关'
          : numeric
            ? String(Number((Number(value) * scale).toFixed(2))) + (key === 'exposure' ? ' EV' : '')
            : '';
    return label + (display ? ` ${display}` : '');
  });
  const detail =
    changes.slice(0, 3).join('，') + (changes.length > 3 ? ` 等 ${changes.length} 项` : '');
  if (!detail) return actions[entry.action] ?? '调整设置';
  return ['undo', 'redo', 'batch'].includes(entry.action)
    ? `${actions[entry.action]} · ${detail}`
    : detail;
}
