import { readScopePixels, noClipping, type Clipping } from './scope-pixels.js';
import { useEffect, useMemo, useState } from 'react';
import type { IrisClient } from '../client.js';
import type { Photo } from '../types.js';
import { usePhotoUrl } from '../react.js';
import { mediaRevision } from '../media-cache.js';
import { ScopeTrace } from './ScopeTrace.js';
import { histogramPath } from './histogram-path.js';
import {
  channels,
  scopeNames,
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
    if (saved && Object.hasOwn(scopeNames, saved.mode)) {
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
  if (!data || (!selected.length && (mode === 'histogram' || mode === 'waveform')))
    return (
      <div className="histogram-empty">
        {!photo ? '未选择照片' : failed ? '示波图暂不可用' : !selected.length ? '选择下方通道' : ''}
      </div>
    );
  if (mode !== 'histogram') return <ScopeTrace data={data} selected={selected} mode={mode} />;
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
            d={histogramPath(data.histogram[channel], peak)}
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
  data,
}: {
  selected: Channel[];
  data: ScopeData | null;
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
          title={
            data && channel !== 'l'
              ? `${channelNames[channel]} · 高端触顶 ${(data.channelClipping[channel].high * 100).toFixed(1)}% · 低端触底 ${(data.channelClipping[channel].low * 100).toFixed(1)}%（单通道，不等同于整块过曝或死黑）`
              : channelNames[channel]
          }
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

function useScopeResult(mediaUrl: string | null, photoId: number | undefined) {
  const [result, setResult] = useState<{
    photoId: number | undefined;
    url: string;
    data: ScopeData | null;
  } | null>(null);
  useEffect(() => {
    if (!mediaUrl) return;
    const url = mediaUrl;
    let active = true;
    void computeScopes(url)
      .then((data) => {
        if (active) setResult({ photoId, url, data });
      })
      .catch(() => {
        if (active) setResult({ photoId, url, data: null });
      });
    return () => {
      active = false;
    };
  }, [mediaUrl, photoId]);
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
  const result = useScopeResult(displayedUrl, photo?.id);
  useEffect(() => {
    try {
      localStorage.setItem(preferenceKey, JSON.stringify({ mode }));
    } catch {
      /* Preferences are optional. */
    }
  }, [mode]);
  // Keep the last complete measurement while this photo's next frame is sampled.
  const data = photo && result?.photoId === photo.id ? result.data : null;
  const failed =
    (!previewUrl && !!media.error) ||
    (!!displayedUrl && result?.url === displayedUrl && !result.data);
  const peak = useMemo(
    () => (data ? histogramPeak(data.histogram, selected) : 1),
    [data, selected]
  );
  const title = scopeNames[mode];
  return (
    <section className="histogram" aria-label="色彩示波器">
      <div className="scope-toolbar">
        <div className="scope-modes" role="group" aria-label="示波器类型">
          {(Object.keys(scopeNames) as ScopeMode[]).map((item) => (
            <button
              type="button"
              key={item}
              aria-pressed={mode === item}
              onClick={() => setSettings((current) => ({ ...current, mode: item }))}
            >
              {scopeNames[item]}
            </button>
          ))}
        </div>
      </div>
      <div
        className={`scope-plot scope-mode-${mode}`}
        role="img"
        aria-label={`预览图${title}，${mode === 'vectorscope' ? 'Cb/Cr 色度，75% 与 100% 色标' : mode === 'parade' ? 'RGB 并列，0–100% 信号刻度' : selected.map((channel) => channelNames[channel]).join('、') || '未选择通道'}，非 RAW 测量`}
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
      {mode === 'histogram' || mode === 'waveform' ? (
        <ScopeChannels selected={selected} onChange={setSettings} data={data} />
      ) : (
        <div className="scope-legend">
          {mode === 'parade' ? 'RGB · 0–100%' : 'Cb/Cr · 75% / 100%'}
        </div>
      )}
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
                : '红色标出 RGB 均接近满值的近白区域；数字为面积占比。单通道触顶见 R/G/B 提示，不代表 RAW 不可恢复'
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
