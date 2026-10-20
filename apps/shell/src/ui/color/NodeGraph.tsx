import { useEffect, useRef, useState } from 'react';
import type { PointerEvent } from 'react';
import type { EditDocument } from '../../editing-client.js';
import { validateGraph } from '../../editing-graph.js';
import {
  addCorrector,
  correctors,
  removeCorrector,
  addMixerBranch,
  disconnectKey,
} from '../../editing-correctors.js';
import { nodeView, viewPositions, type ViewNode, type NodePort } from '../../editing-node-view.js';
import { Button } from '../components.js';
import { NodeCard } from './NodeCard.js';
import {
  connectVisiblePort,
  setCorrectorEnabled,
  nodeCommand,
} from '../../editing-node-actions.js';
import './node-graph.css';
type Connection = { ref: string; key: boolean };
function ConnectionWire({
  nodes,
  positions,
  connection,
  end,
}: {
  nodes: ViewNode[];
  positions: Record<string, number[]>;
  connection: Connection | null;
  end: number[] | null;
}) {
  if (!connection || !end) return null;
  const owner = nodes.find((n) => n.outputs.some((p) => p.ref === connection.ref));
  if (!owner) return null;
  const start = positions[owner.id]!,
    index = owner.outputs.findIndex((p) => p.ref === connection.ref),
    x = start[0]! + 158,
    y = start[1]! + 30 + index * 22;
  return (
    <svg className="node-graph-wires" aria-hidden="true">
      <path className={connection.key ? 'mask-wire' : ''} d={`M ${x} ${y} L ${end[0]} ${end[1]}`} />
    </svg>
  );
}
function fitView(positions: Record<string, number[]>, width: number, height: number) {
  const points = Object.values(positions),
    minX = Math.min(...points.map((p) => p[0]!)),
    minY = Math.min(...points.map((p) => p[1]!));
  const w = Math.max(...points.map((p) => p[0]!)) + 180 - minX,
    h = Math.max(...points.map((p) => p[1]!)) + 100 - minY;
  const scale = Math.max(0.15, Math.min(1, (width / w) * 0.9, (height / h) * 0.9));
  return { x: 16 - minX * scale, y: 16 - minY * scale, scale };
}
function Wires({ nodes, positions }: { nodes: ViewNode[]; positions: Record<string, number[]> }) {
  const owners = new Map(
    nodes.flatMap((n) =>
      n.outputs.flatMap((p, index) =>
        [p.ref, ...(p.aliases ?? [])].map((ref) => [ref, { node: n.id, index }] as const)
      )
    )
  );
  return (
    <svg className="node-graph-wires" aria-hidden="true">
      {nodes.flatMap((n) =>
        n.inputs.map((p, index) => {
          const owner = p.source && owners.get(p.source);
          if (!owner) return null;
          const a = positions[owner.node]!,
            b = positions[n.id]!,
            x = a[0]! + 158,
            y = a[1]! + 30 + owner.index * 22,
            tx = b[0]!,
            ty = b[1]! + 30 + index * 22;
          return (
            <path
              key={n.id + '-' + index}
              className={p.key ? 'mask-wire' : ''}
              d={`M ${x} ${y} L ${tx} ${ty}`}
            />
          );
        })
      )}
    </svg>
  );
}
export function NodeGraph({
  document,
  selected,
  onSelect,
  onChange,
  disabled,
}: {
  document: EditDocument;
  selected: string;
  onSelect(id: string): void;
  onChange(doc: EditDocument): void;
  disabled: boolean;
}) {
  const [view, setView] = useState({ x: 12, y: 12, scale: 0.8 });
  const [connection, setConnection] = useState<Connection | null>(null);
  const [wireEnd, setWireEnd] = useState<number[] | null>(null);
  const wireDrag = useRef<{ x: number; y: number } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [draft, setDraft] = useState<{ id: string; point: number[] } | null>(null);
  const canvas = useRef<HTMLDivElement>(null);
  const gesture = useRef<{ id: string | null; x: number; y: number; base: number[] } | null>(null);
  const nodes = nodeView(document),
    positions = viewPositions(document, nodes),
    groups = correctors(document),
    group = groups.find((g) => g.id === selected);
  if (draft) positions[draft.id] = draft.point;
  const apply = (action: () => EditDocument) => {
    try {
      onChange(validateGraph(action()));
      setError(null);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : '无法修改节点');
    }
  };
  const add = () =>
    apply(() => {
      const next = addCorrector(document, selected);
      onSelect(correctors(next).at(-1)!.id);
      return next;
    });
  const branch = (layer: boolean) => {
    if (!group) return;
    apply(() => {
      const next = addMixerBranch(document, group, layer);
      onSelect(correctors(next).at(-1)!.id);
      return next;
    });
  };
  const remove = () => {
    if (!group) return;
    apply(() => {
      const next = removeCorrector(document, group);
      onSelect(next.output_node);
      return next;
    });
  };
  const connect = (port: NodePort) => {
    if (!connection) return;
    apply(() => connectVisiblePort(document, connection, port));
    setConnection(null);
  };
  const releaseWire = (event: PointerEvent<HTMLDivElement>) => {
    const start = wireDrag.current;
    wireDrag.current = null;
    if (!start || Math.hypot(event.clientX - start.x, event.clientY - start.y) < 3) return;
    const element = event.currentTarget.ownerDocument
      .elementFromPoint(event.clientX, event.clientY)
      ?.closest<HTMLElement>('[data-node-port]');
    const port = nodes
      .flatMap((n) => n.inputs)
      .find(
        (p) =>
          p.target === element?.dataset.nodePort && p.index === Number(element?.dataset.portIndex)
      );
    if (port && connection?.key === port.key) connect(port);
    setWireEnd(null);
  };
  const fit = () =>
    setView(
      fitView(positions, canvas.current?.clientWidth ?? 700, canvas.current?.clientHeight ?? 200)
    );
  // Fit new graph structure; parameter edits and manual node movements retain the view.
  useEffect(() => {
    fit();
  }, [nodes.length]);
  return (
    <section
      className="node-graph"
      aria-label="节点编辑器"
      onKeyDown={(event) => {
        if (
          disabled ||
          (event.target as HTMLElement).matches('input, textarea, [contenteditable="true"]')
        )
          return;
        const command = nodeCommand(event.key, event.altKey, event.ctrlKey || event.metaKey);
        if (command) {
          event.preventDefault();
          event.stopPropagation();
          const actions = {
            serial: add,
            parallel: () => branch(false),
            layer: () => branch(true),
            delete: remove,
            bypass: () => {
              if (group)
                apply(() =>
                  setCorrectorEnabled(
                    document,
                    group,
                    !nodes.find((n) => n.id === group.id)!.enabled
                  )
                );
            },
          };
          actions[command]();
        }
      }}
    >
      <header className="node-graph-bar">
        <Button className="quiet" disabled={disabled} onClick={add} title="添加串行节点（Alt+S）">
          添加节点
        </Button>
        <Button
          className="quiet"
          disabled={disabled || !group}
          onClick={() => branch(false)}
          title="添加并行节点（Alt+P）"
        >
          并行
        </Button>
        <Button
          className="quiet"
          disabled={disabled || !group}
          onClick={() => branch(true)}
          title="添加图层节点（Alt+L）"
        >
          图层
        </Button>
        <Button
          className="quiet"
          disabled={
            disabled ||
            !group ||
            document.panel_bindings.some(
              (b) =>
                ['basic', 'detail', 'geometry'].includes(b.role) &&
                group.members.includes(b.node_id)
            )
          }
          onClick={remove}
        >
          删除
        </Button>
        <span className="spacer" />
        <Button className="quiet" onClick={fit}>
          适合
        </Button>
        <input
          aria-label="节点画布缩放"
          type="range"
          min={0.15}
          max={1.5}
          step={0.05}
          value={view.scale}
          onChange={(e) => setView({ ...view, scale: Number(e.currentTarget.value) })}
        />
      </header>
      <div
        className="node-graph-canvas"
        ref={canvas}
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === 'Escape') {
            setConnection(null);
            setWireEnd(null);
            setError(null);
          }
        }}
        onPointerDown={(e) => {
          if (e.target !== e.currentTarget) return;
          gesture.current = { id: null, x: e.clientX, y: e.clientY, base: [view.x, view.y] };
          e.currentTarget.setPointerCapture(e.pointerId);
        }}
        onPointerMove={(e) => {
          if (connection) {
            const bounds = e.currentTarget.getBoundingClientRect();
            setWireEnd([
              (e.clientX - bounds.left - view.x) / view.scale,
              (e.clientY - bounds.top - view.y) / view.scale,
            ]);
          }
          const g = gesture.current;
          if (!g) return;
          const dx = e.clientX - g.x,
            dy = e.clientY - g.y;
          if (g.id)
            setDraft({
              id: g.id,
              point: [g.base[0]! + dx / view.scale, g.base[1]! + dy / view.scale],
            });
          else setView({ ...view, x: g.base[0]! + dx, y: g.base[1]! + dy });
        }}
        onPointerUp={(event) => {
          releaseWire(event);
          if (draft && !disabled)
            apply(() => ({
              ...document,
              node_positions: { ...document.node_positions, [draft.id]: draft.point },
            }));
          gesture.current = null;
          setDraft(null);
        }}
        onPointerCancel={() => {
          wireDrag.current = null;
          setConnection(null);
          setWireEnd(null);
          gesture.current = null;
          setDraft(null);
        }}
      >
        <div
          className="node-graph-world"
          style={{ transform: `translate(${view.x}px,${view.y}px) scale(${view.scale})` }}
        >
          <Wires nodes={nodes} positions={positions} />
          <ConnectionWire
            nodes={nodes}
            positions={positions}
            connection={connection}
            end={wireEnd}
          />
          {nodes.map((node, index) => (
            <NodeCard
              key={node.id}
              node={node}
              index={nodes.slice(0, index).filter((n) => n.kind === 'corrector').length}
              point={positions[node.id]!}
              selected={selected === node.id}
              disabled={disabled}
              connection={connection}
              onSelect={() => {
                if (node.kind !== 'terminal') onSelect(node.id);
              }}
              onDrag={(e) => {
                if (disabled || node.kind === 'terminal') return;
                e.stopPropagation();
                onSelect(node.id);
                gesture.current = {
                  id: node.id,
                  x: e.clientX,
                  y: e.clientY,
                  base: positions[node.id]!,
                };
                e.currentTarget.setPointerCapture(e.pointerId);
              }}
              onOutput={(ref, key) => setConnection(connection?.ref === ref ? null : { ref, key })}
              onOutputStart={(ref, key, event) => {
                if (disabled) return;
                event.stopPropagation();
                wireDrag.current = { x: event.clientX, y: event.clientY };
                setWireEnd(null);
                setConnection({ ref, key });
                event.currentTarget.setPointerCapture(event.pointerId);
              }}
              onInput={connect}
              onDisconnect={(port) => {
                if (port.key && !port.target.startsWith('__key_'))
                  apply(() => disconnectKey(document, port.target, port.index));
              }}
            />
          ))}
        </div>
      </div>
      {(error || connection) && (
        <div className="node-graph-message" role={error ? 'alert' : 'status'}>
          {error ?? `选择 ${connection?.key ? 'Key' : 'RGB'} 输入，Esc 取消`}
        </div>
      )}
    </section>
  );
}
