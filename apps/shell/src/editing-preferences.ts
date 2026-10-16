import type { EditRecord } from './editing-client.js';
const key = (photo: number) => `iris:editing-version:${photo}`;
export function preferredEdit(photo: number, records: EditRecord[]) {
  try {
    const selected = sessionStorage.getItem(key(photo));
    return (
      records.find((record) => record.id === selected && record.photo_id === photo) ?? records[0]
    );
  } catch {
    return records[0];
  }
}
export function rememberEdit(record: EditRecord) {
  try {
    sessionStorage.setItem(key(record.photo_id), record.id);
  } catch {
    /* Optional view preference. */
  }
}
