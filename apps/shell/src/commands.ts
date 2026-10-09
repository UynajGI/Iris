import type { IrisStore } from './store.js';
export type Command = 'keep' | 'reject' | 'flag' | 'clear' | 'previous' | 'next' | 'undo' | 'review' | 'selectAll'
  | 'groupKeep' | 'groupReject' | 'groupFlag' | 'groupClear' | 'previousGroup' | 'nextGroup'
  | 'previousGroupPhoto' | 'nextGroupPhoto' | 'closeGroup';
export const defaultBindings: Readonly<Record<string, Command>> = Object.freeze({
  w: 'keep', ArrowUp: 'keep', s: 'reject', ArrowDown: 'reject', a: 'previous', ArrowLeft: 'previous',
  d: 'next', ArrowRight: 'next', p: 'flag', z: 'undo', ' ': 'review', 'Control+z': 'undo', 'Meta+z': 'undo',
  'Control+a': 'selectAll', 'Meta+a': 'selectAll',
});
const groupKeyboardCommands: Partial<Record<Command, Command>> = {
  keep: 'groupKeep', reject: 'groupReject', flag: 'groupFlag', clear: 'groupClear',
  previous: 'previousGroupPhoto', next: 'nextGroupPhoto',
};
export class CommandSystem {
  constructor(private readonly store: IrisStore, readonly bindings: Readonly<Record<string, Command>> = defaultBindings) {}
  async dispatch(command: Command): Promise<void> {
    switch (command) {
      case 'keep': return this.store.decide('keep', true);
      case 'reject': return this.store.decide('reject', true);
      case 'flag': return this.store.decide('flag');
      case 'clear': return this.store.decide('pending');
      case 'undo': return this.store.undo();
      case 'previous': return this.store.move(-1);
      case 'next': return this.store.move(1);
      case 'review': return this.store.toggleReview();
      case 'selectAll': return this.store.select(this.store.getSnapshot().photos.map(photo => photo.id));
      case 'groupKeep': return this.store.decideGroup('keep', true);
      case 'groupReject': return this.store.decideGroup('reject', true);
      case 'groupFlag': return this.store.decideGroup('flag');
      case 'groupClear': return this.store.decideGroup('pending');
      case 'previousGroup': return this.store.moveGroup(-1);
      case 'nextGroup': return this.store.moveGroup(1);
      case 'previousGroupPhoto': return this.store.moveGroupPhoto(-1);
      case 'nextGroupPhoto': return this.store.moveGroupPhoto(1);
      case 'closeGroup': return this.store.closeGroup();
    }
  }
  handle(event: KeyboardEvent, onError: (error: unknown) => void = () => {}): boolean {
    const target = event.target as HTMLElement | null;
    if (event.defaultPrevented || event.isComposing || event.repeat || target?.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target?.tagName ?? '')) return false;
    // Space activates the focused native control; it only changes review mode on the canvas.
    if (event.key === ' ' && (target?.tagName === 'BUTTON' || target?.closest?.('button,a,summary,[role="button"]'))) return false;
    const key = event.key.length === 1 ? event.key.toLowerCase() : event.key;
    const chord = `${event.ctrlKey ? 'Control+' : ''}${event.metaKey ? 'Meta+' : ''}${event.altKey ? 'Alt+' : ''}${event.shiftKey ? 'Shift+' : ''}${key}`;
    let command = this.bindings[chord];
    if (!command) return false;
    event.preventDefault();
    const group = this.store.getSnapshot().groupSession;
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
