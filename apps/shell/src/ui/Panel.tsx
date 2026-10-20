import { createContext, useContext, useId, useState, type ReactNode } from 'react';
import { Icon } from './Icon.js';

const storageKey = (id: string) => `iris:panel:${id}:v1`;
const PanelGroupContext = createContext<{
  solo: boolean;
  active: string | null;
  select(id: string | null): void;
} | null>(null);

export function PanelGroup({ children }: { children: ReactNode }) {
  const [solo, setSolo] = useState(false);
  const [active, select] = useState<string | null>('grade-basic');
  return (
    <PanelGroupContext.Provider value={{ solo, active, select }}>
      <div className="grade-panel-options">
        <button type="button" aria-pressed={solo} onClick={() => setSolo(!solo)}>
          单组展开
        </button>
      </div>
      {children}
    </PanelGroupContext.Provider>
  );
}

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
  const group = useContext(PanelGroupContext);
  const [open, setOpen] = useState(() => {
    try {
      const saved = localStorage.getItem(storageKey(id));
      return saved === null ? defaultOpen : saved === '1';
    } catch {
      return defaultOpen;
    }
  });
  const bodyId = useId();
  const expanded = group?.solo ? group.active === id : open;
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
    <section className={`panel ${expanded ? 'open' : ''} ${bypassed ? 'grade-bypassed' : ''}`}>
      <header className="panel-bar">
        <button
          type="button"
          className="panel-toggle"
          aria-expanded={expanded}
          aria-controls={bodyId}
          onClick={() => (group?.solo ? group.select(expanded ? null : id) : toggle())}
        >
          <Icon name="expand" className="panel-caret" />
          <span>{title}</span>
          {modified && <span className="grade-modified" aria-label="已调整" title="已调整" />}
        </button>
        {onBypass && (
          <button
            type="button"
            className="grade-group-compare"
            aria-label={`${bypassed ? '恢复' : '暂时关闭'}${title}效果`}
            aria-pressed={bypassed}
            title={`${bypassed ? '恢复' : '暂时关闭'}${title}效果`}
            onClick={onBypass}
          >
            <Icon name="compare" />
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
