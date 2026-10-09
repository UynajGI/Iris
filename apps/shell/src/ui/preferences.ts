export type Appearance = { size: 'small' | 'medium' | 'large'; density: 'compact' | 'standard' | 'relaxed' };
const defaults: Appearance = { size: 'medium', density: 'standard' };
export function loadAppearance(): Appearance {
  try {
    const data: unknown = JSON.parse(localStorage.getItem('iris:appearance:v1') ?? '{}');
    const input = data as Partial<Appearance> | null;
    return {
      size: input && ['small', 'medium', 'large'].includes(input.size ?? '') ? input.size! : defaults.size,
      density: input && ['compact', 'standard', 'relaxed'].includes(input.density ?? '') ? input.density! : defaults.density,
    };
  } catch { return { ...defaults }; }
}
export function saveAppearance(value: Appearance): void {
  document.documentElement.dataset.size = value.size;
  document.documentElement.dataset.density = value.density;
  try { localStorage.setItem('iris:appearance:v1', JSON.stringify(value)); } catch { /* Session appearance remains usable. */ }
}
