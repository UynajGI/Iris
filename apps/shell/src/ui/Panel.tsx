import { useId, useState, type ReactNode } from 'react';
import { Icon } from './Icon.js';

const storageKey = (id: string) => `iris:panel:${id}:v1`;

/** Collapsible inspector section; open state survives restarts. */
export function Panel({
  id,
  title,
  aside,
  children,
  defaultOpen = true,
}: {
  id: string;
  title: string;
  aside?: ReactNode;
  children: ReactNode;
  defaultOpen?: boolean;
}) {
  const [open, setOpen] = useState(() => {
    try {
      const saved = localStorage.getItem(storageKey(id));
      return saved === null ? defaultOpen : saved === '1';
    } catch {
      return defaultOpen;
    }
  });
  const bodyId = useId();
  const toggle = () =>
    setOpen((value) => {
      try {
        localStorage.setItem(storageKey(id), value ? '0' : '1');
      } catch {
        /* Session state still works. */
      }
      return !value;
    });
  return (
    <section className={`panel ${open ? 'open' : ''}`}>
      <header className="panel-bar">
        <button
          type="button"
          className="panel-toggle"
          aria-expanded={open}
          aria-controls={bodyId}
          onClick={toggle}
        >
          <Icon name="expand" className="panel-caret" />
          <span>{title}</span>
        </button>
        {aside && <span className="panel-aside">{aside}</span>}
      </header>
      <div id={bodyId} className="panel-body" hidden={!open}>
        {children}
      </div>
    </section>
  );
}
