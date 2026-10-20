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
  mountWhenOpen = false,
  modified = false,
  bypassed = false,
  onBypass,
}: {
  id: string;
  title: string;
  aside?: ReactNode;
  children: ReactNode;
  defaultOpen?: boolean;
  mountWhenOpen?: boolean;
  modified?: boolean;
  bypassed?: boolean;
  onBypass?: (() => void) | undefined;
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
  const expanded = open;
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
    <section
      data-panel-id={id}
      className={`panel ${expanded ? 'open' : ''} ${bypassed ? 'grade-bypassed' : ''}`}
    >
      <header className="panel-bar">
        <button
          type="button"
          className="panel-toggle"
          aria-expanded={expanded}
          aria-controls={bodyId}
          onClick={toggle}
        >
          <Icon name="expand" className="panel-caret" />
          <span>{title}</span>
          {modified && <span className="grade-modified" aria-label="已调整" title="已调整" />}
        </button>
        {onBypass && (
          <button
            type="button"
            className="grade-group-compare"
            aria-label={`${title}：${bypassed ? '恢复效果' : '暂时关闭效果'}`}
            aria-pressed={bypassed}
            title={`${title}：${bypassed ? '恢复效果' : '暂时关闭效果'}`}
            onClick={onBypass}
          >
            <Icon name="visibility" />
          </button>
        )}
        {aside && <span className="panel-aside">{aside}</span>}
      </header>
      <div id={bodyId} className="panel-body" hidden={!expanded}>
        {(!mountWhenOpen || expanded) && children}
      </div>
    </section>
  );
}
