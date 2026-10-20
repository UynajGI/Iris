import type { EditDocument } from '../../editing-client.js';
import type { Corrector } from '../../editing-correctors.js';
import { editCorrectorMask, updateCorrectorKey } from '../../editing-node-actions.js';
import { AdjustmentSlider } from './AdjustmentSlider.js';
import { Button } from '../components.js';
import { useState } from 'react';
export function NodeKeyControls({
  document,
  group,
  onChange,
  onEditMask,
}: {
  document: EditDocument;
  group: Corrector;
  onChange(doc: EditDocument): void;
  onEditMask(id: string): void;
}) {
  const [error, setError] = useState<string | null>(null);
  const safely = (action: () => void) => {
    try {
      action();
      setError(null);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : '无法修改选区');
    }
  };
  const key = document.nodes.find((n) => n.id === group.key);
  const params =
    key?.operation.type === 'key_mix' ? key.operation.parameters : { gain: 1, invert: false };
  const change = (gain: number, invert: boolean) =>
    safely(() => onChange(updateCorrectorKey(document, group, gain, invert)));
  const mask = () =>
    safely(() => {
      const next = editCorrectorMask(document, group);
      onChange(next.document);
      onEditMask(next.mask);
    });
  return (
    <div className="node-key-controls">
      {error && <p role="alert">{error}</p>}
      <Button className="quiet" onClick={mask}>
        蒙版与选区
      </Button>
      <label>
        <input
          type="checkbox"
          checked={params.invert}
          onChange={(e) => change(params.gain, e.currentTarget.checked)}
        />
        反转 Key
      </label>
      <AdjustmentSlider
        spec={{ key: 'key_gain', label: 'Key 强度', min: 0, max: 1, scale: 100 }}
        value={params.gain}
        onChange={(value) => change(value, params.invert)}
      />
    </div>
  );
}
