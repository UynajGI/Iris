import { Select } from '../Select.js';
import { useEffect, useRef, useState } from 'react';
import type { IrisClient } from '../../client.js';
import { EditingClient, type EditDocument } from '../../editing-client.js';
import { applyLut, lutNode, removeLut, updateLut, type LutOperation } from '../../editing-lut.js';
import { Panel } from '../Panel.js';
import { Button } from '../components.js';
import { AdjustmentSlider } from './AdjustmentPanels.js';
import { ApiError } from '../../api-error.js';

type Props = {
  client: IrisClient;
  document: EditDocument | null;
  recordId: string | undefined;
  disabled: boolean;
  onChange(document: EditDocument): void;
};
function importError(reason: unknown): string {
  if (!(reason instanceof ApiError))
    return reason instanceof Error ? reason.message : String(reason);
  if (reason.message.includes('quota')) return '调色资源存储空间已达上限';
  if (reason.status === 400) return 'LUT 文件无法导入，请检查是否为有效的 1D / 3D .cube 文件';
  return 'LUT 导入失败，请检查后台连接后重试';
}
function useLutImport(props: Props, domain: LutOperation['parameters']['domain']) {
  const latest = useRef(props);
  latest.current = props;
  const generation = useRef(0);
  const locked = useRef(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  useEffect(() => {
    ++generation.current;
    locked.current = false;
    setBusy(false);
    setError('');
    return () => {
      ++generation.current;
    };
  }, [props.recordId]);
  const upload = async (file: File) => {
    if (locked.current || props.disabled || !props.document) return;
    const current = generation.current;
    locked.current = true;
    setBusy(true);
    setError('');
    try {
      if (!file.name.toLowerCase().endsWith('.cube')) throw new Error('请选择 .cube LUT 文件');
      if (file.size === 0 || file.size > 32 * 1024 * 1024)
        throw new Error('LUT 文件须为 1 字节至 32 MB');
      const resource = await new EditingClient(props.client).importLut(file);
      if (generation.current !== current) return;
      const state = latest.current;
      if (state.disabled || !state.document)
        throw new Error('当前调整正在保存，请完成后重新选择 LUT');
      state.onChange(applyLut(state.document, resource.hash, file.name, domain));
    } catch (reason) {
      if (generation.current === current) setError(importError(reason));
    } finally {
      if (generation.current === current) {
        locked.current = false;
        setBusy(false);
      }
    }
  };
  return { busy, error, upload };
}

export function LutPanel(props: Props) {
  const { document, disabled, onChange } = props;
  const node = lutNode(document);
  const [defaultDomain, setDefaultDomain] =
    useState<LutOperation['parameters']['domain']>('encoded_srgb');
  const domain = node?.operation.parameters.domain ?? defaultDomain;
  const { busy, error, upload } = useLutImport(props, domain);
  return (
    <Panel id="grade-lut" title="LUT">
      <fieldset className="grade-adjustments" disabled={disabled || busy || !document}>
        <legend className="visually-hidden">LUT 调色</legend>
        {node && (
          <label className="grade-output-check">
            <input
              type="checkbox"
              checked={node.enabled}
              onChange={(event) => {
                if (document) onChange(updateLut(document, {}, event.target.checked));
              }}
            />
            启用 LUT
          </label>
        )}
        {node && (
          <p className="filename-line" title={node.name}>
            {node.name}
          </p>
        )}
        <label className="grade-output-field">
          输入色域
          <Select
            aria-label="LUT 输入色域"
            value={domain}
            title="请按 LUT 提供者的说明选择输入色域；不支持相机 Log 转换 LUT。"
            onValueChange={(nextValue) => {
              const next = nextValue as typeof domain;
              setDefaultDomain(next);
              if (document && node) onChange(updateLut(document, { domain: next }));
            }}
          >
            <option value="encoded_srgb">sRGB（非线性）</option>
            <option value="linear_rec2020">线性 Rec.2020</option>
          </Select>
        </label>
        {node && (
          <AdjustmentSlider
            spec={{
              key: 'lut-strength',
              label: '强度',
              min: 0,
              max: 1,
              step: 0.01,
              scale: 100,
              reset: 1,
            }}
            value={node.operation.parameters.strength}
            onChange={(strength) => {
              if (document) onChange(updateLut(document, { strength }));
            }}
          />
        )}
        <div className="grade-lens-actions">
          <label className="grade-resource-import">
            {busy ? '正在导入' : node ? '替换 LUT' : '导入 LUT'}
            <input
              type="file"
              accept=".cube"
              aria-label="导入 LUT"
              onChange={(event) => {
                const file = event.target.files?.[0];
                event.target.value = '';
                if (file) void upload(file);
              }}
            />
          </label>
          {node && (
            <Button
              className="quiet"
              onClick={() => {
                if (document) onChange(removeLut(document));
              }}
            >
              移除
            </Button>
          )}
        </div>
      </fieldset>
      {error && <p role="alert">{error}</p>}
    </Panel>
  );
}
