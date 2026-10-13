import { useState } from 'react';
import type { IrisClient } from '../client.js';
import type { Project } from '../types.js';
import { exportLocation, type ExportFormat, type ExportLocation } from '../export-location.js';
import { Button } from './components.js';

interface CompletedExport {
  projectId: number;
  location: ExportLocation;
  paths: string[];
}

export function useFileExport(client: IrisClient, project: Project | null) {
  const [completed, setCompleted] = useState<CompletedExport | null>(null);
  const run = async (format: ExportFormat, destination: string, scope: string) => {
    if (!project) throw new Error('请先打开照片项目');
    setCompleted(null);
    const location = exportLocation(format, destination, project.root);
    const report =
      format === 'copy'
        ? await client.exportCopy(project.id, destination, scope)
        : format === 'csv'
          ? await client.exportCsv(project.id, destination)
          : await client.exportXmp(project.id, scope, false);
    setCompleted({ projectId: project.id, location, paths: report.paths });
    return `已导出 ${report.written} 项，跳过 ${report.skipped} 项`;
  };
  const result = completed?.projectId === project?.id ? completed : null;
  return { run, result };
}

export function ExportResult({
  result,
  busy,
  onOpen,
}: {
  result: CompletedExport | null;
  busy: boolean;
  onOpen(location: ExportLocation): void;
}) {
  if (!result) return null;
  return (
    <>
      {window.__TAURI__ && (
        <Button className="quiet" disabled={busy} onClick={() => onOpen(result.location)}>
          打开文件夹
        </Button>
      )}
      {result.paths.length > 0 && (
        <details>
          <summary>上次导出位置</summary>
          {result.paths.map((path) => (
            <p className="filename" key={path}>
              {path}
            </p>
          ))}
        </details>
      )}
    </>
  );
}
