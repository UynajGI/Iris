import { useEffect, useMemo, useRef, useState } from 'react';
import type { IrisClient } from '../client.js';
import type { Photo } from '../types.js';
import { usePhotoUrl } from '../react.js';
import { channels, histogramPeak, sampleScopes, type Channel, type ScopeData, type ScopeMode } from './scopes.js';

const preferenceKey = 'iris.scopes.v1';
const channelNames = { l: '白色亮度', r: '红色通道', g: '绿色通道', b: '蓝色通道' };
function preferences(): { mode: ScopeMode; selected: Channel[] } {
  try {
    const saved = JSON.parse(localStorage.getItem(preferenceKey) ?? 'null');
    if (saved && ['histogram', 'waveform'].includes(saved.mode)) {
      return { mode: saved.mode, selected: [...channels] };
    }
  } catch { /* Optional preferences must not prevent viewing a photo. */ }
  return { mode: 'histogram', selected: [...channels] };
}

async function computeScopes(url: string): Promise<ScopeData> {
  const image = new Image();
  image.src = url;
  await image.decode();
  const scale = Math.min(1, 320 / Math.max(image.naturalWidth, image.naturalHeight));
  const width = Math.max(1, Math.round(image.naturalWidth * scale)), height = Math.max(1, Math.round(image.naturalHeight * scale));
  const canvas = document.createElement('canvas');
  canvas.width = width; canvas.height = height;
  const context = canvas.getContext('2d', { willReadFrequently: true });
  if (!context) throw new Error('canvas unavailable');
  context.drawImage(image, 0, 0, width, height);
  return sampleScopes(context.getImageData(0, 0, width, height).data, width, height);
}

function path(bins: Uint32Array, peak: number) {
  let d = 'M0 100';
  for (let i = 0; i < 256; i++) d += `L${i} ${100 - Math.min(1, bins[i]! / peak) * 100}`;
  return `${d}L255 100Z`;
}

function Waveform({ data, selected }: { data: ScopeData; selected: Channel[] }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const context = canvas.current?.getContext('2d');
    if (!context) return;
    const pixels = context.createImageData(data.width, 256);
    const colors = { l: [235, 235, 235], r: [255, 82, 82], g: [85, 235, 129], b: [104, 154, 255] };
    let peak = 1;
    for (const channel of selected) for (const value of data.waveform[channel]) peak = Math.max(peak, value);
    for (let x = 0; x < data.width; x++) for (let level = 0; level < 256; level++) {
      const index = ((255 - level) * data.width + x) * 4;
      for (const channel of selected) {
        const count = data.waveform[channel][x * 256 + level]!;
        if (!count) continue;
        const intensity = 0.2 + 0.8 * Math.log1p(count) / Math.log1p(peak);
        for (let component = 0; component < 3; component++) pixels.data[index + component]! += colors[channel][component]! * intensity;
        pixels.data[index + 3] = 255;
      }
    }
    context.putImageData(pixels, 0, 0);
  }, [data, selected]);
  return <canvas ref={canvas} width={data.width} height={256} className="scope-waveform" aria-hidden="true" />;
}

export function Histogram({ client, photo }: { client: IrisClient; photo: Photo | undefined }) {
  const media = usePhotoUrl(client, photo?.id ?? null, 'preview');
  const [settings, setSettings] = useState(preferences);
  const { mode, selected } = settings;
  const [result, setResult] = useState<{ url: string; data: ScopeData | null } | null>(null);
  useEffect(() => {
    try { localStorage.setItem(preferenceKey, JSON.stringify({ mode })); } catch { /* Preferences are optional. */ }
  }, [mode]);
  useEffect(() => {
    if (!media.url) return;
    const url = media.url;
    let active = true;
    void computeScopes(url).then(data => { if (active) setResult({ url, data }); })
      .catch(() => { if (active) setResult({ url, data: null }); });
    return () => { active = false; };
  }, [media.url]);
  const data = result?.url === media.url ? result?.data : null;
  const failed = !!media.error || !!media.url && result?.url === media.url && !result.data;
  const peak = useMemo(() => data ? histogramPeak(data.histogram, selected) : 1, [data, selected]);
  const title = mode === 'histogram' ? '直方图' : '波形图';
  return <section className="histogram" aria-label="色彩示波器">
    <div className="scope-toolbar">
      <select aria-label="示波器类型" value={mode} onChange={event => setSettings(current => ({ ...current, mode: event.target.value as ScopeMode }))}>
        <option value="histogram">直方图</option><option value="waveform">波形图</option>
      </select>
      <span className="scope-source" title="基于缩小预览图估算，不代表 RAW 原始曝光数据">预览</span>
    </div>
    <div className={`scope-plot scope-mode-${mode}`} role="img" aria-label={`预览图${title}，${selected.map(channel => channelNames[channel]).join('、') || '未选择通道'}，非 RAW 测量`}>
      {data && selected.length > 0 ? mode === 'histogram' ? <svg viewBox="0 0 255 100" preserveAspectRatio="none" aria-hidden="true">
        <g className="histogram-grid">{[0.25, 0.5, 0.75].map(x => <line key={x} x1={x * 255} x2={x * 255} y1={0} y2={100} />)}</g>
        <g className="histogram-curves">{selected.map(channel => <path key={channel} className={`hist-${channel}`} d={path(data.histogram[channel], peak)} strokeDasharray={channel === 'g' ? '3 2' : channel === 'b' ? '1 2' : undefined} />)}</g>
      </svg> : <><Waveform data={data} selected={selected} /><div className="waveform-scale" aria-hidden="true">{[100, 75, 50, 25, 0].map(level => <span key={level}>{level}</span>)}</div></>
        : <div className="histogram-empty">{!photo ? '未选择照片' : failed ? '示波图暂不可用' : !selected.length ? '选择下方通道' : '正在读取预览'}</div>}
    </div>
    <div className="scope-channels" role="group" aria-label="叠加通道">
      {channels.map(channel => <button type="button" key={channel} className={`scope-channel channel-${channel}`} aria-label={channelNames[channel]} aria-pressed={selected.includes(channel)} title={channelNames[channel]} onClick={() => setSettings(current => ({ ...current, selected: current.selected.includes(channel) ? current.selected.filter(value => value !== channel) : channels.filter(value => value === channel || current.selected.includes(value)) }))}>{channel === 'l' ? 'W' : channel.toUpperCase()}</button>)}
    </div>
    <div className="histogram-meta tabular">
      <span className={data && data.clipLow > 0.01 ? 'clip-warn' : ''}>暗部 {data ? `${(data.clipLow * 100).toFixed(1)}%` : '—'}</span>
      <span className={data && data.clipHigh > 0.01 ? 'clip-warn' : ''}>高光 {data ? `${(data.clipHigh * 100).toFixed(1)}%` : '—'}</span>
    </div>
  </section>;
}
