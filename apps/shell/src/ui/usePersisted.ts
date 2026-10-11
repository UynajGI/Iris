import { useEffect, useState } from 'react';

function readStored(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}
export function usePersisted<T extends number | boolean>(
  key: string,
  fallback: NoInfer<T>,
  valid: (value: T) => boolean = () => true
) {
  const [value, setValue] = useState<T>(() => {
    const raw = readStored(key);
    if (raw === null) return fallback;
    const parsed = (typeof fallback === 'boolean' ? raw === '1' : Number(raw)) as T;
    return valid(parsed) ? parsed : fallback;
  });
  useEffect(() => {
    try {
      localStorage.setItem(key, typeof value === 'boolean' ? (value ? '1' : '0') : String(value));
    } catch {
      /* Session value still applies. */
    }
  }, [key, value]);
  return [value, setValue] as const;
}
