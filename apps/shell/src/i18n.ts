export type Locale = 'zh-CN' | 'en';
export function localeFor(language: string): Locale { return language.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en'; }
export function productName(language: string): string { return localeFor(language) === 'zh-CN' ? '伊人' : 'IrisVision'; }
