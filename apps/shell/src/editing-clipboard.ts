import type { EditDocument, EditRecipe } from './editing-client.js';
import { applyColorPreset, colorPresetRecipe } from './editing-presets.js';

/** Window-local snapshot. Uses the same portable controls and merge as saved presets. */
export class AdjustmentClipboard {
  private recipe: EditRecipe | null = null;

  get available() {
    return this.recipe !== null;
  }

  copy(document: EditDocument) {
    this.recipe = colorPresetRecipe(document);
  }

  paste(document: EditDocument): EditDocument {
    if (!this.recipe) throw new Error('请先复制调整');
    return applyColorPreset(document, this.recipe);
  }
}

export function adjustmentClipboardShortcut(event: KeyboardEvent): 'copy' | 'paste' | null {
  const target = event.target as HTMLElement | null;
  if (
    event.defaultPrevented ||
    event.repeat ||
    event.isComposing ||
    event.altKey ||
    event.shiftKey ||
    !(event.ctrlKey || event.metaKey) ||
    target?.isContentEditable ||
    /^(INPUT|TEXTAREA|SELECT)$/.test(target?.tagName ?? '') ||
    target?.closest?.('dialog,[role="dialog"],[role="listbox"]')
  )
    return null;
  const key = event.key.toLowerCase();
  return key === 'c' ? 'copy' : key === 'v' ? 'paste' : null;
}
