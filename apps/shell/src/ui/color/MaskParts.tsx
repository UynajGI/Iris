import { Select } from '../Select.js';
import { useState } from 'react';
import { maskNames, type MaskDefinition, type MaskKind } from '../../editing-masks.js';
import {
  appendMaskPart,
  combineNames,
  maskParts,
  removeMaskPart,
  type MaskCombine,
} from '../../editing-mask-parts.js';
import { Button } from '../components.js';

export function MaskParts({
  definition,
  selected,
  onSelect,
  onChange,
}: {
  definition: MaskDefinition;
  selected: number;
  onSelect(index: number): void;
  onChange(definition: MaskDefinition): boolean;
}) {
  const parts = maskParts(definition);
  const [operation, setOperation] = useState<MaskCombine>('add');
  const [error, setError] = useState('');
  const current = Math.min(selected, parts.length - 1);
  return (
    <div className="grade-mask-parts">
      <div className="grade-mask-part-list" role="group" aria-label="蒙版组成">
        {parts.map((part, index) => (
          <Button
            key={part.path}
            className="quiet"
            aria-pressed={index === current}
            onClick={() => onSelect(index)}
          >
            <span>{part.relation}</span>
            <span>
              {maskNames[part.definition.kind]} {index + 1}
            </span>
          </Button>
        ))}
      </div>
      <div className="segmented" role="group" aria-label="蒙版组合方式">
        {Object.entries(combineNames).map(([value, name]) => (
          <Button
            key={value}
            aria-pressed={operation === value}
            onClick={() => setOperation(value as MaskCombine)}
          >
            {name}
          </Button>
        ))}
      </div>
      <Select
        aria-label="新增蒙版组成"
        value=""
        onValueChange={(nextValue) => {
          if (!nextValue) return;
          try {
            if (onChange(appendMaskPart(definition, operation, nextValue as MaskKind)))
              onSelect(parts.length);
            setError('');
          } catch (reason) {
            setError(reason instanceof Error ? reason.message : String(reason));
          }
        }}
      >
        <option value="">{combineNames[operation]}画笔或选区</option>
        {Object.entries(maskNames).map(([kind, name]) => (
          <option key={kind} value={kind}>
            {name}
          </option>
        ))}
      </Select>
      {parts.length > 1 && (
        <Button
          className="quiet"
          onClick={() => {
            try {
              if (onChange(removeMaskPart(definition, parts[current]!.path)))
                onSelect(Math.max(0, current - 1));
              setError('');
            } catch (reason) {
              setError(reason instanceof Error ? reason.message : String(reason));
            }
          }}
        >
          移除选中组成
        </Button>
      )}
      {error && (
        <p role="alert" className="inline-error">
          {error}
        </p>
      )}
    </div>
  );
}
