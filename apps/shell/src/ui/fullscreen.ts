import { useCallback, useEffect, useState, type RefObject } from 'react';

/** Element fullscreen through the webview; Escape exits natively. */
export function useFullscreen(target: RefObject<HTMLElement | null>) {
  const [active, setActive] = useState(false);
  useEffect(() => {
    const sync = () => setActive(!!document.fullscreenElement && document.fullscreenElement === target.current);
    document.addEventListener('fullscreenchange', sync);
    return () => document.removeEventListener('fullscreenchange', sync);
  }, [target]);
  const toggle = useCallback(async () => {
    if (document.fullscreenElement) await document.exitFullscreen();
    else await target.current?.requestFullscreen({ navigationUI: 'hide' });
  }, [target]);
  return { active, toggle, supported: typeof document !== 'undefined' && document.fullscreenEnabled };
}
