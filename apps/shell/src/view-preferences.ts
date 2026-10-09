import type { PhotoFilter } from './types.js';

export interface PreferenceStorage { getItem(key: string): string | null; setItem(key: string, value: string): void }
export interface ViewPreferences { filter: PhotoFilter; reviewOpen: boolean }
const prefix = 'iris:view-preferences:v1:';
const defaults = (): ViewPreferences => ({ filter: { limit: 200 }, reviewOpen: false });
const record = (value: unknown): Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value) ? value as Record<string, unknown> : {};

/** Only user-selectable filter values are retained; offset is an ephemeral cursor. */
export function validatedPhotoFilter(value: unknown, strict = false): PhotoFilter {
  const input = record(value);
  const output: Record<string, unknown> = { limit: 200 };
  const choices: Record<string, readonly string[]> = {
    decision: ['pending', 'keep', 'reject', 'flag'], verdict: ['recommend', 'review', 'reject_suggest'],
    color_label: ['none', 'red', 'yellow', 'green', 'blue', 'purple'],
    sort: ['name', 'size', 'date', 'score', 'suggestion'], format: ['jpeg', 'jpg', 'png', 'webp', 'raw', 'heic', 'heif', 'live'],
  };
  const invalid = (key: string) => { if (strict) throw new Error(`Invalid photo filter: ${key}`); };
  for (const [key, allowed] of Object.entries(choices)) {
    if (input[key] === undefined) continue;
    if (typeof input[key] === 'string' && allowed.includes(input[key])) output[key] = input[key]; else invalid(key);
  }
  for (const key of ['descending', 'include_missing']) {
    if (input[key] === undefined) continue;
    if (typeof input[key] === 'boolean') output[key] = input[key]; else invalid(key);
  }
  if (input.limit !== undefined) {
    if (Number.isSafeInteger(input.limit) && Number(input.limit) >= 1 && Number(input.limit) <= 1000) output.limit = input.limit; else invalid('limit');
  }
  if (input.rating !== undefined) {
    if (Number.isSafeInteger(input.rating) && Number(input.rating) >= 0 && Number(input.rating) <= 5) output.rating = input.rating;
    else invalid('rating');
  }
  if (strict && input.offset !== undefined) {
    if (Number.isSafeInteger(input.offset) && Number(input.offset) >= 0) output.offset = input.offset; else invalid('offset');
  }
  return output as PhotoFilter;
}

function browserStorage(): PreferenceStorage | undefined {
  try { return typeof window === 'undefined' ? undefined : window.localStorage; } catch { return undefined; }
}

/** Storage failures degrade to process-local preferences, never to a failed photo operation. */
export class ViewPreferencesRepository {
  private memory = new Map<string, ViewPreferences>();
  constructor(private readonly storage: PreferenceStorage | undefined = browserStorage()) {}
  load(projectRoot: string): ViewPreferences {
    const cached = this.memory.get(projectRoot);
    if (cached) return structuredClone(cached);
    let preferences = defaults();
    try {
      const text = this.storage?.getItem(prefix + encodeURIComponent(projectRoot));
      if (text && text.length <= 8192) {
        const stored = record(JSON.parse(text));
        if (stored.version === 1) preferences = { filter: validatedPhotoFilter(stored.filter), reviewOpen: stored.reviewOpen === true };
      }
    } catch { /* Malformed, unavailable, or blocked storage uses defaults. */ }
    this.memory.set(projectRoot, preferences);
    return structuredClone(preferences);
  }
  save(projectRoot: string, preferences: ViewPreferences): void {
    const safe = { filter: validatedPhotoFilter(preferences.filter), reviewOpen: preferences.reviewOpen === true };
    this.memory.set(projectRoot, safe);
    try { this.storage?.setItem(prefix + encodeURIComponent(projectRoot), JSON.stringify({ version: 1, ...safe })); } catch { /* Quota/security failure retains the memory copy. */ }
  }
}
