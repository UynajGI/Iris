import type { IrisStore } from './store.js';
export type Command =
  | 'keep'
  | 'reject'
  | 'flag'
  | 'clear'
  | 'previous'
  | 'next'
  | 'undo'
  | 'review'
  | 'selectAll'
  | 'groupKeep'
  | 'groupReject'
  | 'groupFlag'
  | 'groupClear'
  | 'previousGroup'
  | 'nextGroup'
  | 'previousGroupPhoto'
  | 'nextGroupPhoto'
  | 'closeGroup';
export const defaultBindings: Readonly<Record<string, Command>> = Object.freeze({
  w: 'keep',
  ArrowUp: 'keep',
  s: 'reject',
  ArrowDown: 'reject',
  a: 'previous',
  ArrowLeft: 'previous',
  d: 'next',
  ArrowRight: 'next',
  p: 'flag',
  z: 'undo',
  ' ': 'review',
  'Control+z': 'undo',
  'Meta+z': 'undo',
  'Control+a': 'selectAll',
  'Meta+a': 'selectAll',
});
const groupKeyboardCommands: Partial<Record<Command, Command>> = {
  keep: 'groupKeep',
  reject: 'groupReject',
  flag: 'groupFlag',
  clear: 'groupClear',
  previous: 'previousGroupPhoto',
  next: 'nextGroupPhoto',
};
export class CommandSystem {
  private readonly handlers: Record<Command, () => void | Promise<void>>;
  constructor(
    private readonly store: IrisStore,
    readonly bindings: Readonly<Record<string, Command>> = defaultBindings
  ) {
    this.handlers = commandHandlers(store);
  }
  async dispatch(command: Command): Promise<void> {
    return this.handlers[command]?.();
  }
  handle(event: KeyboardEvent, onError: (error: unknown) => void = () => {}): boolean {
    const target = event.target as HTMLElement | null;
    if (
      event.defaultPrevented ||
      event.isComposing ||
      event.repeat ||
      target?.isContentEditable ||
      /^(INPUT|TEXTAREA|SELECT)$/.test(target?.tagName ?? '')
    )
      return false;
    // Space activates the focused native control; it only changes review mode on the canvas.
    if (
      event.key === ' ' &&
      (target?.tagName === 'BUTTON' || target?.closest?.('button,a,summary,[role="button"]'))
    )
      return false;
    const key = event.key.length === 1 ? event.key.toLowerCase() : event.key;
    const chord = `${event.ctrlKey ? 'Control+' : ''}${event.metaKey ? 'Meta+' : ''}${event.altKey ? 'Alt+' : ''}${event.shiftKey ? 'Shift+' : ''}${key}`;
    let command = this.bindings[chord];
    if (!command) return false;
    event.preventDefault();
    const state = this.store.getSnapshot();
    const group = state.groupSession;
    if (command === 'selectAll' && state.reviewOpen && !group) return true;
    if (command === 'review' && group) {
      this.store.closeGroup();
      return true;
    }
    if (group) {
      // Keyboard focus follows the active group; explicit dispatch retains its
      // library semantics. Never fall back to the library while members load.
      if (group.loading && command !== 'closeGroup') return true;
      // Groups have a focused photo and comparison candidates, not a bulk
      // selection. Do not let Ctrl/Cmd+A select the hidden library page.
      if (command === 'selectAll') return true;
      command = groupKeyboardCommands[command] ?? command;
    }
    void this.dispatch(command).catch(onError);
    return true;
  }
  attach(target: Window, onError?: (error: unknown) => void): () => void {
    const handler = (event: KeyboardEvent) => this.handle(event, onError);
    target.addEventListener('keydown', handler);
    return () => target.removeEventListener('keydown', handler);
  }
}

/** The type-checked table makes every command's side effect explicit in review. */
function commandHandlers(store: IrisStore): Record<Command, () => void | Promise<void>> {
  return {
    keep: () => store.decide('keep', true),
    reject: () => store.decide('reject', true),
    flag: () => store.decide('flag'),
    clear: () => store.decide('pending'),
    undo: () => store.undo(),
    previous: () => store.move(-1),
    next: () => store.move(1),
    review: () => store.toggleReview(),
    selectAll: () => store.select(store.getSnapshot().photos.map((photo) => photo.id)),
    groupKeep: () => store.decideGroup('keep', true),
    groupReject: () => store.decideGroup('reject', true),
    groupFlag: () => store.decideGroup('flag'),
    groupClear: () => store.decideGroup('pending'),
    previousGroup: () => store.moveGroup(-1),
    nextGroup: () => store.moveGroup(1),
    previousGroupPhoto: () => store.moveGroupPhoto(-1),
    nextGroupPhoto: () => store.moveGroupPhoto(1),
    closeGroup: () => store.closeGroup(),
  };
}
