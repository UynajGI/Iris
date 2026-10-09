/** Native folder selection is explicit; cancellation never changes the project. */
export interface NativeDialogBridge {
  invoke<T>(command: string): Promise<T>;
}

export async function selectProjectFolder(bridge: NativeDialogBridge): Promise<string | null> {
  const selected = await bridge.invoke<unknown>('select_project_folder');
  if (selected === null) return null;
  if (typeof selected !== 'string' || selected.length === 0) throw new Error('Invalid native folder selection');
  return selected;
}

export async function selectExportCsv(bridge: NativeDialogBridge): Promise<string | null> {
  const selected = await bridge.invoke<unknown>('select_export_csv');
  if (selected === null) return null;
  if (typeof selected !== 'string' || selected.length === 0) throw new Error('Invalid native CSV selection');
  return selected;
}

export async function openProjectFolder(
  bridge: NativeDialogBridge,
  application: { openProject(root: string): Promise<void> },
): Promise<boolean> {
  const selected = await selectProjectFolder(bridge);
  if (selected === null) return false;
  await application.openProject(selected);
  return true;
}
