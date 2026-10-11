export type WorkspaceShortcut =
  | 'panels'
  | 'help'
  | 'grid'
  | 'loupe'
  | 'faces'
  | 'fullscreen'
  | 'zoomIn'
  | 'zoomOut'
  | 'zoomToggle'
  | 0
  | 1
  | 2
  | 3
  | 4
  | 5;

export function workspaceShortcut(event: KeyboardEvent): WorkspaceShortcut | null {
  const target = event.target as HTMLElement | null;
  if (
    event.defaultPrevented ||
    event.repeat ||
    event.isComposing ||
    event.ctrlKey ||
    event.metaKey ||
    event.altKey ||
    target?.isContentEditable ||
    /^(INPUT|TEXTAREA|SELECT)$/.test(target?.tagName ?? '') ||
    target?.closest?.('dialog,[role="dialog"]')
  )
    return null;
  if (event.key === '?') return 'help';
  if (event.shiftKey && event.key.toLowerCase() === 'z') return 'zoomToggle';
  if (event.key === '+' || event.key === '=') return 'zoomIn';
  if (event.key === '-' || event.key === '_') return 'zoomOut';
  if (event.shiftKey) return null;
  if (/^[0-5]$/.test(event.key)) return Number(event.key) as 0 | 1 | 2 | 3 | 4 | 5;
  switch (event.key.toLowerCase()) {
    case 'b':
      return 'panels';
    case 'g':
      return 'grid';
    case 'e':
      return 'loupe';
    case 'i':
      return 'faces';
    case 'f':
      return 'fullscreen';
    default:
      return null;
  }
}
