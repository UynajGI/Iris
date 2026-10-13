import { Fragment } from 'react';
import { Icon } from './Icon.js';

export type AppPage = 'projects' | 'photos' | 'color' | 'retouch' | 'export' | 'settings' | 'help';
const stages = [
  ['projects', '导入'],
  ['photos', '选片'],
  ['color', '调色'],
  ['retouch', '修图'],
  ['export', '导出'],
] as const;

export function WorkflowNav({
  page,
  hasProject,
  onNavigate,
}: {
  page: AppPage;
  hasProject: boolean;
  onNavigate(page: AppPage): void;
}) {
  return (
    <nav className="modules workflow-nav" aria-label="照片工作流程">
      {stages.map(([target, label], index) => (
        <Fragment key={target}>
          {index > 0 && <Icon name="chevron-right" className="workflow-divider" />}
          <button
            type="button"
            className="module"
            disabled={target !== 'projects' && !hasProject}
            aria-current={page === target ? 'page' : undefined}
            onClick={() => onNavigate(target)}
          >
            {label}
          </button>
        </Fragment>
      ))}
    </nav>
  );
}
