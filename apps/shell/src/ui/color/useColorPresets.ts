import { useEffect, useMemo, useRef, useState } from 'react';
import type { IrisClient } from '../../client.js';
import { EditingClient, type EditPreset } from '../../editing-client.js';

export function useColorPresets(client: IrisClient) {
  const api = useMemo(() => new EditingClient(client), [client]);
  const [items, setItems] = useState<EditPreset[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [more, setMore] = useState(false);
  const offset = useRef(0);
  const active = useRef(false);
  const generation = useRef(0);
  const read = useRef<AbortController | null>(null);
  const load = async (append = false) => {
    if (active.current) return;
    active.current = true;
    const epoch = generation.current;
    const controller = new AbortController();
    read.current = controller;
    setBusy(true);
    setError('');
    const start = append ? offset.current : 0;
    try {
      const page = await api.presets(start, controller.signal);
      if (epoch !== generation.current) return;
      setItems((current) =>
        append ? [...new Map([...current, ...page].map((item) => [item.id, item])).values()] : page
      );
      offset.current = start + page.length;
      setMore(page.length === 100);
    } catch (reason) {
      if (epoch === generation.current)
        setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      if (epoch === generation.current) {
        active.current = false;
        setBusy(false);
      }
    }
  };
  useEffect(() => {
    generation.current++;
    active.current = false;
    setItems([]);
    void load();
    return () => {
      generation.current++;
      read.current?.abort();
    };
  }, [api]);
  const mutate = async (action: () => Promise<EditPreset | null>, removedId?: string) => {
    if (active.current) return null;
    active.current = true;
    const epoch = generation.current;
    setBusy(true);
    setError('');
    try {
      const result = await action();
      if (epoch !== generation.current) return null;
      setItems((current) => {
        const kept = current.filter((item) => item.id !== (result?.id ?? removedId));
        return result ? [result, ...kept] : kept;
      });
      // Names determine pagination order. Refresh after a mutation before requesting more pages.
      setMore(false);
      offset.current = 0;
      const page = await api.presets();
      if (epoch !== generation.current) return null;
      setItems([
        ...new Map([...(result ? [result] : []), ...page].map((item) => [item.id, item])).values(),
      ]);
      offset.current = page.length;
      setMore(page.length === 100);
      return { result };
    } catch (reason) {
      if (epoch === generation.current)
        setError(
          `${reason instanceof Error ? reason.message : String(reason)}。请刷新列表确认结果后再试。`
        );
      return null;
    } finally {
      if (epoch === generation.current) {
        active.current = false;
        setBusy(false);
      }
    }
  };
  return { api, items, busy, error, more, load, mutate };
}
