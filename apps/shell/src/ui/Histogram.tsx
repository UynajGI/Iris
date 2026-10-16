import { readScopePixels, noClipping, type Clipping } from './scope-pixels.js';
import { useEffect, useMemo, useRef, useState } from 'react';
import type { IrisClient } from '../client.js';
import type { Photo } from '../types.js';
import { usePhotoUrl } from '../react.js';
import { mediaRevision } from '../media-cache.js';
import {
  channels,
  histogramPeak,
  sampleScopes,
  type Channel,
  type ScopeData,
  type ScopeMode,
} from './scopes.js';

const preferenceKey = 'iris.scopes.v1';
const channelNames = { l: 'Y 亮度通道', r: '红色通道', g: '绿色通道', b: '蓝色通道' };
function preferences(): { mode: ScopeMode; selected: Channel[] } {
  try {
    const saved = JSON.parse(localStorage.getItem(preferenceKey) ?? 'null');
    if (saved && ['histogram', 'waveform'].includes(saved.mode)) {
      return { mode: saved.mode, selected: [...channels] };
    }
  } catch {
    /* Optional preferences must not prevent viewing a photo. */
  }
  return { mode: 'histogram', selected: [...channels] };
}

async function computeScopes(url: string): Promise<ScopeData> {
  const pixels = await readScopePixels(url);
  return sampleScopes(pixels.data, pixels.width, pixels.height);
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
    for (const channel of selected)
      for (const value of data.waveform[channel]) peak = Math.max(peak, value);
    for (let x = 0; x < data.width; x++)
      for (let level = 0; level < 256; level++) {
        const index = ((255 - level) * data.width + x) * 4;
        for (const channel of selected) {
          const count = data.waveform[channel][x * 256 + level]!;
          if (!count) continue;
          const intensity =
            (0.1 + (0.8 * Math.log1p(count)) / Math.log1p(peak)) *
            (channel === 'l' && selected.length > 1 ? 0.45 : 1);
          for (let component = 0; component < 3; component++) {
            const previous = pixels.data[index + component]!;
            pixels.data[index + component] =
              255 - (255 - previous) * (1 - (colors[channel][component]! * intensity) / 255);
          }
          pixels.data[index + 3] = 255;
        }
      }
    context.putImageData(pixels, 0, 0);
  }, [data, selected]);
  return (
    <canvas
      ref={canvas}
      width={data.width}
      height={256}
      className="scope-waveform"
      aria-hidden="true"
    />
  );
}

function ScopePlot({
  data,
  selected,
  mode,
  peak,
  photo,
  failed,
}: {
  data: ScopeData | null;
  selected: Channel[];
  mode: ScopeMode;
  peak: number;
  photo: Photo | undefined;
  failed: boolean;
}) {
  if (!data || !selected.length)
    return (
      <div className="histogram-empty">
        {!photo
          ? '未选择照片'
          : failed
            ? '示波图暂不可用'
            : !selected.length
              ? '选择下方通道'
              : '正在读取预览'}
      </div>
    );
  if (mode !== 'histogram')
    return (
      <>
        <Waveform data={data} selected={selected} />
        <div className="waveform-scale" aria-hidden="true">
          {[100, 75, 50, 25, 0].map((level) => (
            <span key={level}>{level}</span>
          ))}
        </div>
      </>
    );
  return (
    <svg viewBox="0 0 255 100" preserveAspectRatio="none" aria-hidden="true">
      <g className="histogram-grid">
        {[0.25, 0.5, 0.75].map((x) => (
          <line key={x} x1={x * 255} x2={x * 255} y1={0} y2={100} />
        ))}
      </g>
      <g className="histogram-curves">
        {selected.map((channel) => (
          <path
            key={channel}
            className={`hist-${channel}`}
            d={path(data.histogram[channel], peak)}
            vectorEffect="non-scaling-stroke"
          />
        ))}
      </g>
    </svg>
  );
}

function ScopeChannels({
  selected,
  onChange,
}: {
  selected: Channel[];
  onChange: (
    update: (current: { mode: ScopeMode; selected: Channel[] }) => {
      mode: ScopeMode;
      selected: Channel[];
    }
  ) => void;
}) {
  return (
    <div className="scope-channels" role="group" aria-label="叠加通道">
      {channels.map((channel) => (
        <button
          type="button"
          key={channel}
          className={`scope-channel channel-${channel}`}
          aria-label={channelNames[channel]}
          aria-pressed={selected.includes(channel)}
          title={channelNames[channel]}
          onClick={() =>
            onChange((current) => ({
              ...current,
              selected: current.selected.includes(channel)
                ? current.selected.filter((value) => value !== channel)
                : channels.filter((value) => value === channel || current.selected.includes(value)),
            }))
          }
        >
          {channel === 'l' ? 'Y' : channel.toUpperCase()}
        </button>
      ))}
    </div>
  );
}

function useScopeResult(mediaUrl: string | null) {
  const [result, setResult] = useState<{ url: string; data: ScopeData | null } | null>(null);
  useEffect(() => {
    if (!mediaUrl) return;
    const url = mediaUrl;
    let active = true;
    void computeScopes(url)
      .then((data) => {
        if (active) setResult({ url, data });
      })
      .catch(() => {
        if (active) setResult({ url, data: null });
      });
    return () => {
      active = false;
    };
  }, [mediaUrl]);
  return result;
}

export function Histogram({
  client,
  photo,
  previewUrl,
  clipping = noClipping,
  onClipping,
}: {
  client: IrisClient;
  photo: Photo | undefined;
  previewUrl?: string | null | undefined;
  clipping?: Clipping;
  onClipping?: ((value: Clipping) => void) | undefined;
}) {
  const media = usePhotoUrl(
    client,
    photo?.id ?? null,
    'preview',
    photo ? mediaRevision(photo) : ''
  );
  const [settings, setSettings] = useState(preferences);
  const { mode, selected } = settings;
  const displayedUrl = previewUrl ?? media.url;
  const result = useScopeResult(displayedUrl);
  useEffect(() => {
    try {
      localStorage.setItem(preferenceKey, JSON.stringify({ mode }));
    } catch {
      /* Preferences are optional. */
    }
  }, [mode]);
  const data = result?.url === displayedUrl ? result?.data : null;
  const failed =
    (!previewUrl && !!media.error) ||
    (!!displayedUrl && result?.url === displayedUrl && !result.data);
  const peak = useMemo(
    () => (data ? histogramPeak(data.histogram, selected) : 1),
    [data, selected]
  );
  const title = mode === 'histogram' ? '直方图' : '波形图';
  return (
    <section className="histogram" aria-label="色彩示波器">
      <div className="scope-toolbar">
        <div className="scope-modes" role="group" aria-label="示波器类型">
          {(['histogram', 'waveform'] as const).map((item) => (
            <button
              type="button"
              key={item}
              aria-pressed={mode === item}
              onClick={() => setSettings((current) => ({ ...current, mode: item }))}
            >
              {item === 'histogram' ? '直方图' : '波形图'}
            </button>
          ))}
        </div>
        <span className="scope-source" title="基于缩小预览图估算，不代表 RAW 原始曝光数据">
          预览
        </span>
      </div>
      <div
        className={`scope-plot scope-mode-${mode}`}
        role="img"
        aria-label={`预览图${title}，${selected.map((channel) => channelNames[channel]).join('、') || '未选择通道'}，非 RAW 测量`}
      >
        <ScopePlot
          data={data}
          selected={selected}
          mode={mode}
          peak={peak}
          photo={photo}
          failed={failed}
        />
      </div>
      <ScopeChannels selected={selected} onChange={setSettings} />
      <ScopeClipping data={data} clipping={clipping} onChange={onClipping} />
    </section>
  );
}

function ScopeClipping({
  data,
  clipping,
  onChange,
}: {
  data: ScopeData | null;
  clipping: Clipping;
  onChange: ((value: Clipping) => void) | undefined;
}) {
  return (
    <div className="histogram-meta tabular">
      {(['shadows', 'highlights'] as const).map((kind) => {
        const label = kind === 'shadows' ? '暗部' : '高光';
        const ratio = kind === 'shadows' ? data?.clipLow : data?.clipHigh;
        const text = (
          <>
            {label} <span>{ratio === undefined ? '—' : `${(ratio * 100).toFixed(1)}%`}</span>
          </>
        );
        return onChange ? (
          <button
            key={kind}
            type="button"
            className={`scope-clipping ${kind}`}
            aria-label={`${label}裁切提示`}
            aria-pressed={clipping[kind]}
            disabled={!data}
            title={
              kind === 'shadows'
                ? '蓝色标出 RGB 均接近 0 的预览像素；数字为面积占比'
                : '红色标出任一 RGB 通道接近满值的预览像素；数字为面积占比，不代表 RAW 不可恢复'
            }
            onClick={() => onChange({ ...clipping, [kind]: !clipping[kind] })}
          >
            {text}
          </button>
        ) : (
          <span key={kind}>{text}</span>
        );
      })}
    </div>
  );
}
