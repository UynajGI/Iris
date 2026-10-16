import { useEffect, useState } from 'react';
import type { EditDocument } from '../../editing-client.js';
import { AdjustmentClipboard, adjustmentClipboardShortcut } from '../../editing-clipboard.js';
import { Button } from '../components.js';

// Survives switching photos, projects and workflow pages in this window; never uses the OS clipboard.
const clipboard = new AdjustmentClipboard();

export function AdjustmentClipboardControls({
  document,
  disabled,
  onChange,
}: {
  document: EditDocument | null;
  disabled: boolean;
  onChange(document: EditDocument): void;
}) {
  const [copied, setCopied] = useState<EditDocument | null>(null);
  const [error, setError] = useState<{ document: EditDocument; message: string } | null>(null);
  const act = (action: 'copy' | 'paste') => {
    if (!document || disabled) return;
    try {
      if (action === 'copy') {
        clipboard.copy(document);
        setCopied(document);
      } else {
        onChange(clipboard.paste(document));
      }
      setError(null);
    } catch (reason) {
      setError({ document, message: reason instanceof Error ? reason.message : String(reason) });
    }
  };
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (disabled || !document || window.document.querySelector('dialog[open],[role="dialog"]'))
        return;
      const action = adjustmentClipboardShortcut(event);
      if (!action || (action === 'paste' && !clipboard.available)) return;
      event.preventDefault();
      act(action);
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  });
  return (
    <>
      <div className="grade-clipboard" role="group" aria-label="复制与粘贴全局调整">
        <Button
          className="quiet"
          disabled={disabled || !document}
          aria-label="复制调整"
          aria-keyshortcuts="Control+Shift+C Meta+Shift+C"
          title="复制全局影调、颜色与细节（Ctrl/⌘+Shift+C）"
          onClick={() => act('copy')}
        >
          {copied && copied === document ? '已复制' : '复制调整'}
        </Button>
        <Button
          className="quiet"
          disabled={disabled || !document || !clipboard.available}
          aria-label="粘贴调整"
          aria-keyshortcuts="Control+Shift+V Meta+Shift+V"
          title="粘贴全局影调、颜色与细节（Ctrl/⌘+Shift+V）"
          onClick={() => act('paste')}
        >
          粘贴调整
        </Button>
      </div>
      {error?.document === document && (
        <p className="grade-error" role="alert">
          {error.message}
        </p>
      )}
    </>
  );
}
