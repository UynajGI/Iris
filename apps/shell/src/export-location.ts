export type ExportFormat = 'copy' | 'csv' | 'xmp';
export interface ExportLocation {
  path: string;
  file: boolean;
}
/** Keep the completed export's destination, not the currently edited dialog fields. */
export function exportLocation(
  format: ExportFormat,
  destination: string,
  projectRoot: string
): ExportLocation {
  return { path: format === 'xmp' ? projectRoot : destination, file: format === 'csv' };
}
export async function openExportLocation(location: ExportLocation): Promise<void> {
  if (!window.__TAURI__) throw new Error('请在桌面应用中打开文件夹');
  await window.__TAURI__.core.invoke('open_export_folder', { ...location });
}
