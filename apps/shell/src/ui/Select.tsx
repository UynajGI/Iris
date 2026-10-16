import {
  Children,
  isValidElement,
  useCallback,
  useState,
  type ButtonHTMLAttributes,
  type ReactNode,
} from 'react';
import * as Primitive from '@radix-ui/react-select';

type Option = { value: string; label: ReactNode; disabled?: boolean };

function optionsFrom(children: ReactNode): Option[] {
  return Children.toArray(children).flatMap((child) => {
    if (!isValidElement<{ value: string | number; children: ReactNode; disabled?: boolean }>(child))
      return [];
    return [
      {
        value: String(child.props.value),
        label: child.props.children,
        disabled: !!child.props.disabled,
      },
    ];
  });
}

type SelectProps = Omit<
  ButtonHTMLAttributes<HTMLButtonElement>,
  'value' | 'onChange' | 'children'
> & {
  value: string | number;
  onValueChange(value: string): void;
  children: ReactNode;
};

/** Shared single-choice control. Options remain declarative; no native popup is rendered. */
export function Select({
  value,
  onValueChange,
  children,
  disabled,
  name,
  className = '',
  ...props
}: SelectProps) {
  const options = optionsFrom(children);
  const [open, setOpen] = useState(false);
  const [container, setContainer] = useState<HTMLElement | null>(null);
  const mount = useCallback((node: HTMLButtonElement | null) => {
    // Keep the popup in a native dialog's top layer and inside parent outside-click boundaries.
    if (node) setContainer(node.closest<HTMLElement>('dialog, .filter-popover'));
  }, []);
  const current = options.find((item) => item.value === String(value)) ?? options[0];
  return (
    <Primitive.Root
      open={open}
      onOpenChange={setOpen}
      value={`v:${current?.value ?? ''}`}
      onValueChange={(next) => onValueChange(next.slice(2))}
      disabled={disabled}
    >
      <Primitive.Trigger
        {...props}
        ref={mount}
        className={`iris-select ${className}`}
        onKeyDown={(event) => {
          if (event.key !== 'Escape') event.stopPropagation();
        }}
      >
        <Primitive.Value>{current?.label ?? '暂无选项'}</Primitive.Value>
        <Primitive.Icon className="iris-select-chevron" aria-hidden="true">
          <svg viewBox="0 0 16 16">
            <path d="m4.5 6.5 3.5 3.5 3.5-3.5" />
          </svg>
        </Primitive.Icon>
      </Primitive.Trigger>
      <Primitive.Portal container={container}>
        <Primitive.Content
          className="iris-select-menu"
          aria-label={props['aria-label'] ?? '选择选项'}
          aria-labelledby={props['aria-labelledby']}
          position="popper"
          align="start"
          sideOffset={5}
          collisionPadding={8}
          onKeyDown={(event) => {
            event.stopPropagation();
            if (event.key === 'Escape') {
              event.preventDefault();
              setOpen(false);
            }
          }}
        >
          <Primitive.ScrollUpButton className="iris-select-scroll" aria-label="向上滚动">
            <svg viewBox="0 0 16 16" aria-hidden="true">
              <path d="m4.5 9.5 3.5-3.5 3.5 3.5" />
            </svg>
          </Primitive.ScrollUpButton>
          <Primitive.Viewport className="iris-select-viewport">
            {options.map((item) => (
              <Primitive.Item
                key={item.value}
                value={`v:${item.value}`}
                disabled={item.disabled}
                className="iris-select-option"
              >
                <Primitive.ItemText>{item.label}</Primitive.ItemText>
                <Primitive.ItemIndicator className="iris-select-check" aria-hidden="true">
                  <svg viewBox="0 0 16 16">
                    <path d="m3.5 8 3 3 6-6" />
                  </svg>
                </Primitive.ItemIndicator>
              </Primitive.Item>
            ))}
          </Primitive.Viewport>
          <Primitive.ScrollDownButton className="iris-select-scroll" aria-label="向下滚动">
            <svg viewBox="0 0 16 16" aria-hidden="true">
              <path d="m4.5 6.5 3.5 3.5 3.5-3.5" />
            </svg>
          </Primitive.ScrollDownButton>
        </Primitive.Content>
      </Primitive.Portal>
      {name && <input type="hidden" name={name} value={current?.value ?? ''} disabled={disabled} />}
    </Primitive.Root>
  );
}
