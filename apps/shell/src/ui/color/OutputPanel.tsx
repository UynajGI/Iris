import type { EditDocument } from '../../editing-client.js';
import { outputSpaceNames } from '../../editing-document.js';
import { AdjustmentSlider } from './AdjustmentPanels.js';

export function OutputPanel({
  document,
  disabled,
  onChange,
}: {
  document: EditDocument | null;
  disabled: boolean;
  onChange(document: EditDocument): void;
}) {
  if (!document) return null;
  const output = document.output;
  const change = (value: Partial<typeof output>) =>
    onChange({ ...document, output: { ...output, ...value } });
  return (
    <section className="grade-settings-page" aria-label="输出与颜色管理">
      <fieldset className="grade-adjustments" disabled={disabled}>
        <legend className="visually-hidden">输出与颜色管理</legend>
        <label className="grade-output-field">
          输出色域
          <select
            aria-label="输出色域"
            value={output.space}
            onChange={(event) => change({ space: event.target.value as typeof output.space })}
          >
            {Object.entries(outputSpaceNames).map(([value, name]) => (
              <option value={value} key={value}>
                {name}
              </option>
            ))}
          </select>
        </label>
        <label className="grade-output-field">
          高光输出
          <select
            aria-label="高光输出"
            value={output.tone_map.kind}
            onChange={(event) =>
              change({
                tone_map:
                  event.target.value === 'shoulder'
                    ? { kind: 'shoulder', knee: 0.8 }
                    : { kind: 'none' },
              })
            }
          >
            <option value="none">直接输出</option>
            <option value="shoulder">柔和压缩</option>
          </select>
        </label>
        {output.tone_map.kind === 'shoulder' && (
          <AdjustmentSlider
            spec={{
              key: 'knee',
              label: '压缩起点',
              min: 0,
              max: 0.99,
              step: 0.01,
              scale: 100,
              reset: 0.8,
            }}
            value={output.tone_map.knee}
            onChange={(knee) => change({ tone_map: { kind: 'shoulder', knee } })}
          />
        )}
        <label className="grade-output-check">
          <input
            type="checkbox"
            checked={output.gamut_compression}
            onChange={(event) => change({ gamut_compression: event.target.checked })}
          />
          压缩超出色域的颜色
        </label>
      </fieldset>
    </section>
  );
}
