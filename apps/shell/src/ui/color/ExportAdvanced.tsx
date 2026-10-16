import { useEffect, useMemo, useRef, useState } from 'react';
import type { IrisClient } from '../../client.js';
import { EditingClient, type EditDocument, type EditRecord } from '../../editing-client.js';
import { EditingPreviewQueue } from '../../editing-preview-queue.js';
import { PreviewFrames } from '../../preview-frames.js';
import { OutputPanel } from './OutputPanel.js';
import { LutPanel } from './LutPanel.js';
import { ProofPanel, type ProofProfile } from './ProofPanel.js';

type Props = {
  client: IrisClient;
  record: EditRecord;
  document: EditDocument;
  disabled: boolean;
  onChange(document: EditDocument): void;
};
type PreviewInput = {
  record: EditRecord;
  document: EditDocument;
  proof: string | null;
  sequence: number;
  edge: number;
  mode: string;
};

function useExportPreview(props: Props, proof: string | null) {
  const api = useMemo(() => new EditingClient(props.client), [props.client]);
  const session = useMemo(() => crypto.randomUUID(), [props.record.id]);
  const sequence = useRef(0);
  const queue = useRef<EditingPreviewQueue<PreviewInput, Blob> | null>(null);
  const [frame, setFrame] = useState<{ url: string; mode: string } | null>(null);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const mode = JSON.stringify([props.record.id, props.record.revision, proof]);
  const latestMode = useRef(mode);
  latestMode.current = mode;
  useEffect(() => {
    const frames = new PreviewFrames();
    const pending = new EditingPreviewQueue<PreviewInput, Blob>({
      render: (input, signal) =>
        api.preview(
          input.record,
          input.document,
          session,
          input.sequence,
          signal,
          input.edge,
          input.proof
        ),
      result: (blob, input) => {
        void frames.show(
          blob,
          (url) => {
            if (input.mode !== latestMode.current) return false;
            setFrame({ url, mode: input.mode });
            setError('');
            return true;
          },
          (reason) => setError(String(reason))
        );
      },
      error: (reason) => setError(reason instanceof Error ? reason.message : String(reason)),
      busy: setBusy,
      cancel: (input) => {
        void api.cancel(session, input.sequence).catch(() => {});
      },
    });
    queue.current = pending;
    return () => {
      pending.dispose();
      frames.dispose();
      void api.close(session).catch(() => {});
    };
  }, [api, session]);
  useEffect(() => {
    const { record, document } = props;
    setError('');
    queue.current?.push({ record, document, proof, mode, sequence: ++sequence.current, edge: 512 });
    const timer = setTimeout(
      () =>
        queue.current?.push({
          record,
          document,
          proof,
          mode,
          sequence: ++sequence.current,
          edge: 1200,
        }),
      300
    );
    return () => clearTimeout(timer);
  }, [props.record, props.document, proof, mode]);
  return { preview: frame?.mode === mode ? frame.url : null, busy, error };
}

function ExportColorControls(props: Props) {
  const [profile, setProfile] = useState<ProofProfile | null>(null);
  const { preview, busy, error } = useExportPreview(props, profile?.enabled ? profile.hash : null);
  return (
    <div className="export-color-layout">
      <div className="export-color-preview" aria-label="调整与软打样预览" aria-busy={busy}>
        {preview ? (
          <img src={preview} alt={profile?.enabled ? '软打样预览' : '调整预览'} />
        ) : (
          <span>{error ? '预览不可用' : ''}</span>
        )}
        {preview && (
          <span className="export-preview-label">{profile?.enabled ? '软打样' : '调整预览'}</span>
        )}
        {error && <p role="alert">{error}</p>}
      </div>
      <div className="export-color-controls">
        <OutputPanel
          document={props.document}
          disabled={props.disabled}
          onChange={props.onChange}
        />
        <LutPanel
          client={props.client}
          document={props.document}
          recordId={props.record.id}
          disabled={props.disabled}
          onChange={props.onChange}
        />
        <ProofPanel
          client={props.client}
          profile={profile}
          disabled={props.disabled}
          onChange={setProfile}
        />
      </div>
    </div>
  );
}

export function ExportAdvanced(props: Props) {
  const [open, setOpen] = useState(false);
  return (
    <details
      className="export-advanced"
      open={open}
      onToggle={(event) => setOpen(event.currentTarget.open)}
    >
      <summary>
        高级<span>颜色管理</span>
      </summary>
      {open && <ExportColorControls {...props} />}
    </details>
  );
}
