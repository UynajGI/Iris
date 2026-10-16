import { useEffect, useMemo, useRef, useState } from 'react';
import type { IrisClient } from '../../client.js';
import {
  EditingClient,
  type EditDocument,
  type EditRecord,
  type RawWhiteBalanceRequest,
  type PreparedRawWhiteBalance,
  type WhiteBalanceTemperature,
} from '../../editing-client.js';
import { applyWhiteBalance, whiteBalanceKey } from '../../editing-white-balance.js';
import { EditingPreviewQueue } from '../../editing-preview-queue.js';

type Props = {
  client: IrisClient;
  record: EditRecord | null;
  document: EditDocument | null;
  enabled: boolean;
  onChange(document: EditDocument): void;
};
type Input = {
  record: EditRecord;
  key: string;
  request: RawWhiteBalanceRequest;
  apply: boolean;
  serial: number;
};

export function useRawWhiteBalance(props: Props) {
  const api = useMemo(() => new EditingClient(props.client), [props.client]);
  const latest = useRef(props);
  latest.current = props;
  const serial = useRef(0);
  const queue = useRef<EditingPreviewQueue<Input, PreparedRawWhiteBalance> | null>(null);
  const readingKey = useRef('');
  const [reading, setReading] = useState<WhiteBalanceTemperature | null>(null);
  const [loading, setLoading] = useState(false);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState('');
  const [retry, setRetry] = useState(0);
  const key = props.record && props.document ? whiteBalanceKey(props.record, props.document) : '';
  const identity = `${props.enabled}:${props.record?.id}:${props.record?.revision}`;
  useEffect(() => {
    readingKey.current = '';
    setReading(null);
    setError('');
    setPending(false);
    setLoading(false);
    const jobs = new EditingPreviewQueue<Input, PreparedRawWhiteBalance>({
      render: (input, signal) => api.rawWhiteBalance(input.record, input.request, signal),
      result: (result, input) => {
        const now = latest.current;
        if (
          input.serial !== serial.current ||
          !now.enabled ||
          !now.record ||
          !now.document ||
          whiteBalanceKey(now.record, now.document) !== input.key
        )
          return;
        const document = input.apply
          ? applyWhiteBalance(now.record, now.document, result, input.request.kind === 'as_shot')
          : now.document;
        readingKey.current = whiteBalanceKey(now.record, document);
        setReading(result.temperature ?? null);
        setError('');
        if (input.apply) now.onChange(document);
      },
      error: (reason) => {
        setError(reason instanceof Error ? reason.message : String(reason));
        setReading(null);
        readingKey.current = '';
      },
      busy: (value) => {
        setLoading(value);
        if (!value) setPending(false);
      },
      cancel: () => {},
    });
    queue.current = jobs;
    return () => {
      ++serial.current;
      jobs.dispose();
      queue.current = null;
    };
  }, [api, identity]);
  useEffect(() => {
    if (!props.enabled || !props.record || !props.document || readingKey.current === key) return;
    const gains = props.document.source.raw_white_balance;
    setReading(null);
    queue.current?.push(
      {
        record: props.record,
        key,
        apply: false,
        serial: ++serial.current,
        request: gains
          ? { kind: 'gains', gains_rgb: gains, locus: 'planckian' }
          : { kind: 'as_shot' },
      },
      true
    );
  }, [key, identity, retry]);
  const request = (request: RawWhiteBalanceRequest) => {
    const now = latest.current;
    if (!now.enabled || !now.record || !now.document) return;
    setError('');
    setPending(true);
    if (request.kind === 'temperature')
      setReading({
        kelvin: request.kelvin,
        tint_delta_v: request.tint_delta_v,
        locus: request.locus,
      });
    queue.current?.push({
      record: now.record,
      key: whiteBalanceKey(now.record, now.document),
      request,
      apply: true,
      serial: ++serial.current,
    });
  };
  return { reading, loading, pending, error, request, retry: () => setRetry((value) => value + 1) };
}
export type RawWhiteBalanceModel = ReturnType<typeof useRawWhiteBalance>;
