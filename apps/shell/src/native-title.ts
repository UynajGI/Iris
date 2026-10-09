import { localeFor, productName, type Locale } from './i18n.js';

export interface NativeTitleBridge {
  invoke<T>(command: string, args: { locale: Locale }): Promise<T>;
}

/** Serialize locale changes; the native command accepts only the two product names. */
export function createAppTitleSynchronizer(bridge: NativeTitleBridge, document: Pick<Document, 'title'>) {
  let queue: Promise<void> = Promise.resolve();
  return (language: string): Promise<void> => {
    const locale = localeFor(language);
    const next = queue.catch(() => {}).then(async () => {
      await bridge.invoke<void>('set_app_locale', { locale });
      document.title = productName(locale);
    });
    queue = next;
    return next;
  };
}
