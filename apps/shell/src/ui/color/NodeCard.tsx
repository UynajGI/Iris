import type { PointerEvent } from 'react';
import { type ViewNode, type NodePort, viewKind } from '../../editing-node-view.js';
export function NodeCard({
  node,
  index,
  point,
  selected,
  disabled,
  onSelect,
  onDrag,
  onInput,
  onDisconnect,
  onOutput,
  onOutputStart,
  connection,
}: {
  node: ViewNode;
  index: number;
  point: number[];
  selected: boolean;
  disabled: boolean;
  onSelect(): void;
  onDrag(event: PointerEvent<HTMLButtonElement>): void;
  onInput(port: NodePort): void;
  onDisconnect(port: NodePort): void;
  onOutput(ref: string, key: boolean): void;
  onOutputStart(ref: string, key: boolean, event: PointerEvent<HTMLButtonElement>): void;
  connection: { ref: string; key: boolean } | null;
}) {
  return (
    <div
      className={`graph-node ${selected ? 'selected' : ''} ${node.enabled ? '' : 'bypassed'}`}
      style={{
        left: point[0],
        top: point[1],
        minHeight: Math.max(84, 38 + node.inputs.length * 18),
      }}
    >
      <button
        className="graph-node-title"
        aria-pressed={selected}
        onClick={onSelect}
        onPointerDown={onDrag}
      >
        {node.kind === 'corrector' && <small>{String(index + 1).padStart(2, '0')}</small>}
        {node.name}
      </button>
      <span className="graph-node-kind">{viewKind(node)}</span>
      {node.inputs.map((port, index) => (
        <button
          key={index}
          className={`graph-port input ${port.key ? 'mask-port' : ''}`}
          style={{ top: 24 + index * 22 }}
          aria-label={`${node.name} ${port.label}输入`}
          data-node-port={port.target}
          data-port-index={port.index}
          title={`${port.label} 输入${port.key ? '；右键断开' : ''}`}
          disabled={disabled}
          onClick={() => {
            if (connection?.key === port.key) onInput(port);
          }}
          onContextMenu={(event) => {
            event.preventDefault();
            if (!disabled) onDisconnect(port);
          }}
        />
      ))}
      {node.outputs.map((port, index) => (
        <button
          key={port.ref}
          className={`graph-port output ${port.key ? 'mask-port' : ''} ${connection?.ref === port.ref ? 'connecting' : ''}`}
          style={{ top: 24 + index * 22 }}
          aria-label={`${node.name} ${port.key ? 'Key' : 'RGB'}输出`}
          title={`${port.key ? 'Key' : 'RGB'} 输出`}
          disabled={disabled}
          onPointerDown={(event) => onOutputStart(port.ref, port.key, event)}
          onClick={(event) => {
            if (event.detail === 0) onOutput(port.ref, port.key);
          }}
        />
      ))}
    </div>
  );
}
