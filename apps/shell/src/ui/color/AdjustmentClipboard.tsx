import { useEffect, useState } from 'react';
import type { EditDocument } from '../../editing-client.js';
import { AdjustmentClipboard, adjustmentClipboardShortcut } from '../../editing-clipboard.js';

// Survives switching photos, projects and workflow pages in this window; never uses the OS clipboard.
const clipboard = new AdjustmentClipboard();

export function AdjustmentClipboardShortcuts({
  document,
  disabled,
  onChange,
}: {
  document: EditDocument | null;
  disabled: boolean;
  onChange(document: EditDocument): void;
}) {
  const [error, setError] = useState<{ document: EditDocument; message: string } | null>(null);
  const act = (action: 'copy' | 'paste') => {
    if (!document || disabled) return;
    try {
      if (action === 'copy') {
        clipboard.copy(document);
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
      if (action === 'copy' && window.getSelection()?.toString()) return;
      event.preventDefault();
      act(action);
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  });
  return error?.document === document ? (
    <p className="grade-error" role="alert">
      {error.message}
    </p>
  ) : null;
}
