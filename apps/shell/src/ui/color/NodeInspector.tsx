import type { EditDocument } from '../../editing-client.js';
import { type EditNode, nodeNames, validateGraph } from '../../editing-graph.js';
import { AdjustmentPanels, detail } from './AdjustmentPanels.js';
import { useState } from 'react';
import { AdjustmentSlider } from './AdjustmentSlider.js';
import { Button } from '../components.js';
import { Icon } from '../Icon.js';
import { correctors, correctorDocument, updateCorrector } from '../../editing-correctors.js';
import { NodeKeyControls } from './NodeKeyControls.js';
import { setCorrectorEnabled } from '../../editing-node-actions.js';

export function NodeInspector({
  document,
  node,
  disabled,
  onChange,
  onEditMask,
  onCrop,
}: {
  document: EditDocument;
  node: EditNode;
  disabled: boolean;
  onChange(document: EditDocument): void;
  onEditMask(id: string): void;
  onCrop(): void;
}) {
  const kind = node.operation.type;
  const group = correctors(document).find((g) => g.id === node.id);
  const [error, setError] = useState<string | null>(null);
  const safely = (action: () => EditDocument) => {
    if (disabled) return;
    try {
      onChange(validateGraph(action()));
      setError(null);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : '无法修改节点');
    }
  };
  const edit = (updated: EditNode) =>
    safely(() => ({
      ...document,
      nodes: document.nodes.map((n) => (n.id === node.id ? updated : n)),
    }));
  const role = kind === 'adjustments' ? 'basic' : kind;
  const projected = {
    ...document,
    panel_bindings: [
      ...document.panel_bindings.filter((b) => b.role !== role && b.role !== `traditional.${kind}`),
      { role, node_id: node.id },
    ],
  };
  return (
    <div className="node-inspector" data-kind={group ? 'corrector' : kind}>
      <header className="node-inspector-heading">
        <input
          aria-label="节点名称"
          maxLength={80}
          value={group?.name ?? node.name}
          disabled={disabled}
          onChange={(e) => edit({ ...node, name: e.currentTarget.value })}
        />
        <Button
          className="quiet icon-only"
          aria-label="节点效果开关"
          aria-pressed={node.enabled}
          disabled={
            disabled || kind === 'source' || kind === 'mask' || (!group && kind === 'geometry')
          }
          onClick={() =>
            group
              ? safely(() => setCorrectorEnabled(document, group, !node.enabled))
              : edit({ ...node, enabled: !node.enabled })
          }
        >
          <Icon name="visibility" />
        </Button>
      </header>
      {error && <p role="alert">{error}</p>}
      <fieldset disabled={disabled} className="node-parameters">
        {group && (
          <NodeKeyControls
            document={document}
            group={group}
            onChange={(next) => safely(() => next)}
            onEditMask={onEditMask}
          />
        )}
        {group ? (
          <AdjustmentPanels
            document={correctorDocument(document, group)}
            onCrop={onCrop}
            disabled={disabled}
            onChange={(next) => safely(() => updateCorrector(document, group, next))}
          />
        ) : node.operation.type === 'detail' ? (
          <fieldset disabled={disabled}>
            {detail.map((spec) => (
              <AdjustmentSlider
                key={spec.key}
                spec={spec}
                value={
                  (node.operation as Extract<EditNode['operation'], { type: 'detail' }>).parameters[
                    spec.key as 'texture'
                  ]
                }
                onChange={(value) => {
                  if (node.operation.type === 'detail')
                    edit({
                      ...node,
                      operation: {
                        ...node.operation,
                        parameters: { ...node.operation.parameters, [spec.key]: value },
                      },
                    });
                }}
              />
            ))}
          </fieldset>
        ) : (['adjustments', 'geometry'] as string[]).includes(kind) ? (
          <AdjustmentPanels
            nodeKind={kind}
            document={projected}
            onCrop={onCrop}
            disabled={disabled}
            onChange={(next) => {
              const updated = next.nodes.find((n) => n.id === node.id);
              if (updated) edit(updated);
            }}
          />
        ) : node.operation.type === 'blend' ? (
          <AdjustmentSlider
            spec={{ key: 'opacity', label: '混合强度', min: 0, max: 1, scale: 100 }}
            value={node.operation.parameters.opacity}
            onChange={(value) =>
              edit({ ...node, operation: { type: 'blend', parameters: { opacity: value } } })
            }
          />
        ) : node.operation.type === 'lut' ? (
          <AdjustmentSlider
            spec={{ key: 'strength', label: 'LUT 强度', min: 0, max: 1, scale: 100 }}
            value={node.operation.parameters.strength}
            onChange={(value) => {
              if (node.operation.type === 'lut')
                edit({
                  ...node,
                  operation: {
                    ...node.operation,
                    parameters: { ...node.operation.parameters, strength: value },
                  },
                });
            }}
          />
        ) : (
          <p className="muted">
            {kind === 'mask' ? '在蒙版分页编辑选区。' : `${nodeNames[kind]}输入`}
          </p>
        )}
      </fieldset>
    </div>
  );
}
