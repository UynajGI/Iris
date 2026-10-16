type EditingNavigation = {
  getSnapshot(): { groupSession: { memberIds: number[] } | null };
  focus(id: number): void;
  focusGroupPhoto(id: number): void;
};
/** Re-read group membership after a pending save; never focus the unrelated library behind it. */
export function focusEditingPhoto(store: EditingNavigation, id: number): boolean {
  const group = store.getSnapshot().groupSession;
  if (group) {
    if (!group.memberIds.includes(id)) return false;
    store.focusGroupPhoto(id);
  } else store.focus(id);
  return true;
}
