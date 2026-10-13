import { APP_VERSION } from './generated/version.js';

export interface SupportInfo {
  version: string;
  build: string;
  platform: string;
  architecture: string;
  distribution: string;
  debug: boolean;
  logs_available: boolean;
  daemon_version: string | null;
  data_directory: string;
  log_directory: string;
}
export type DiagnosticEvent =
  | 'frontend_failed'
  | 'render_failed'
  | 'connection_lost'
  | 'task_started'
  | 'task_completed'
  | 'task_failed'
  | 'task_cancelled'
  | 'update_failed';
export type SupportLink = 'homepage' | 'releases' | 'issues' | 'logs';
export const supportLinks: Record<Exclude<SupportLink, 'logs'>, string> = {
  homepage: 'https://github.com/UynajGI/Iris',
  releases: 'https://github.com/UynajGI/Iris/releases',
  issues: 'https://github.com/UynajGI/Iris/issues/new/choose',
};
export function recordDiagnostic(event: DiagnosticEvent): void {
  if (typeof window !== 'undefined')
    void window.__TAURI__?.core.invoke('record_diagnostic_event', { event }).catch(() => {});
}
export async function openSupportLink(target: SupportLink): Promise<void> {
  if (window.__TAURI__) return window.__TAURI__.core.invoke('open_support_link', { target });
  if (target === 'logs') throw new Error('请在桌面应用中打开日志');
  window.open(supportLinks[target], '_blank', 'noopener,noreferrer');
}
export function versionText(info?: SupportInfo): string {
  return info
    ? `Iris / 伊人 ${info.version}\n构建：${info.build}\n系统：${info.platform} ${info.architecture}\n发行：${info.distribution}${info.debug ? ' (debug)' : ''}\n界面版本：${APP_VERSION}\n后台版本：${info.daemon_version ?? '不可用'}`
    : `Iris / 伊人 ${APP_VERSION}\n运行方式：浏览器预览`;
}

/** Sharing is explicit. The full error remains local; clipboard text omits free-form values. */
export function errorReport(error: string, info?: SupportInfo): string {
  const category = /permission|denied|权限/i.test(error)
    ? '权限不足'
    : /timeout|timed out|超时/i.test(error)
      ? '操作超时'
      : /network|fetch|connect|连接|网络/i.test(error)
        ? '服务连接失败'
        : /not found|missing|不存在|找不到/i.test(error)
          ? '文件或组件缺失'
          : /space|disk|磁盘|空间/i.test(error)
            ? '存储错误'
            : '操作失败';
  return `${versionText(info)}\n时间：${new Date().toISOString()}\n错误类别：${category}\n为保护隐私，原始错误文字未复制。可附加诊断文件和复现步骤。`;
}
export async function copyText(text: string): Promise<void> {
  if (!navigator.clipboard?.writeText)
    throw new Error('当前环境无法访问剪贴板，请手动选择并复制文字');
  await navigator.clipboard.writeText(text);
}
